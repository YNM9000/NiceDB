use crate::session::Sql as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use nd_driver::{Chunk, Connection, Dialect, DriverError, Row};
use nd_transfer::{Batch, FileFormat, Inferred, ReadOptions, Record, Unit, WriteOptions};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::{Pace, Progress, ToolFault, ms};
use crate::guard::{Access, collect, quiet};
use crate::{Core, CoreError, Result};

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataExportSource {
    Table { namespace: String, name: String },
    Query { sql: String },
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct DataExport {
    pub job_id: String,
    pub session_id: String,
    pub source: DataExportSource,
    pub path: String,
    pub format: FileFormat,
    pub header: bool,
    pub expected_rows: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DataExported {
    #[specta(type = f64)]
    pub rows: u64,
    #[specta(type = f64)]
    pub bytes: u64,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct DataImportSource {
    pub path: String,
    pub format: FileFormat,
    pub header: bool,
    pub sheet: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SampleColumn {
    pub name: String,
    pub inferred: Inferred,
    pub type_name: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DataImportSample {
    pub columns: Vec<SampleColumn>,
    pub rows: Vec<Row>,
    pub sheets: Vec<String>,
    pub more: bool,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct NewColumn {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataImportTarget {
    Existing {
        namespace: String,
        name: String,
    },
    Create {
        namespace: String,
        name: String,
        columns: Vec<NewColumn>,
    },
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ColumnMap {
    pub source: u32,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OnRowError {
    Stop,
    Skip,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct DataImport {
    pub job_id: String,
    pub session_id: String,
    pub source: DataImportSource,
    pub target: DataImportTarget,
    pub mapping: Vec<ColumnMap>,
    pub empty_is_null: bool,
    pub on_error: OnRowError,
    pub confirm: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RowFault {
    #[specta(type = f64)]
    pub line: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DataImported {
    #[specta(type = f64)]
    pub inserted: u64,
    #[specta(type = f64)]
    pub skipped: u64,
    pub faults: Vec<RowFault>,
    pub faults_cut: bool,
    pub committed: bool,
    pub cancelled: bool,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
}

const SAMPLE_ROWS: usize = 1000;
const PREVIEW_ROWS: usize = 50;
const MAX_FAULTS: usize = 500;
const READ_BATCH: usize = 2000;

pub(crate) fn qualified(dialect: Dialect, namespace: &str, name: &str) -> String {
    if namespace.is_empty() {
        nd_sql::ident(dialect, name)
    } else {
        format!(
            "{}.{}",
            nd_sql::ident(dialect, namespace),
            nd_sql::ident(dialect, name)
        )
    }
}

fn max_params(dialect: Dialect) -> usize {
    match dialect {
        Dialect::Mssql => 2000,
        Dialect::Oracle => 1000,
        _ => 30_000,
    }
}

fn placeholder(dialect: Dialect, n: usize) -> String {
    match dialect {
        Dialect::Postgres | Dialect::Duckdb => format!("${n}"),
        Dialect::Mssql => format!("@P{n}"),
        Dialect::Oracle => format!(":{n}"),
        Dialect::Mysql
        | Dialect::Sqlite
        | Dialect::Clickhouse
        | Dialect::Firebird
        | Dialect::Hana
        | Dialect::Db2
        | Dialect::Ansi
        | Dialect::AnsiBacktick
        | Dialect::Cql
        | Dialect::Partiql
        | Dialect::Opensearch
        | Dialect::Elasticsearch => "?".into(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn read_options(source: &DataImportSource) -> ReadOptions {
    ReadOptions {
        header: source.header,
        sheet: source.sheet.clone(),
    }
}

fn checked_path(path: &str, format: FileFormat) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if FileFormat::from_path(&path) != Some(format) {
        return Err(ToolFault::WrongExtension.into());
    }
    Ok(path)
}

pub(crate) fn single_read(sql: &str, dialect: Dialect) -> Result<String> {
    let statements = nd_sql::split(sql, dialect);
    match statements.as_slice() {
        [st] if nd_sql::is_read(st.text(sql), dialect) => Ok(st.text(sql).to_owned()),
        _ => Err(ToolFault::NotRead.into()),
    }
}

pub(super) fn normalize(dialect: Dialect, type_name: Option<&str>, v: String) -> String {
    let boolean = type_name.is_some_and(|t| {
        matches!(nd_transfer::kind_of(Some(t)), nd_transfer::Kind::Boolean)
            || t.eq_ignore_ascii_case("boolean")
    });
    if !boolean || matches!(dialect, Dialect::Postgres | Dialect::Duckdb) {
        return v;
    }
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "t" | "yes" | "y" | "si" | "sì" => "1".into(),
        "false" | "f" | "no" | "n" => "0".into(),
        _ => v,
    }
}

pub(super) struct Plan {
    dialect: Dialect,
    table: String,
    columns: Vec<String>,
    types: Vec<Option<String>>,
    sources: Vec<usize>,
    empty_is_null: bool,
    per_statement: usize,
}

impl Plan {
    pub(super) fn direct(
        dialect: Dialect,
        table: String,
        columns: Vec<String>,
        types: Vec<Option<String>>,
    ) -> Self {
        let width = columns.len();
        Self {
            dialect,
            table,
            sources: (0..width).collect(),
            columns,
            types,
            empty_is_null: false,
            per_statement: (max_params(dialect) / width.max(1)).clamp(1, 500),
        }
    }

    fn of(dialect: Dialect, table: String, req: &DataImport, types: Vec<Option<String>>) -> Self {
        Self {
            dialect,
            table,
            columns: req.mapping.iter().map(|m| m.target.clone()).collect(),
            types,
            sources: req.mapping.iter().map(|m| m.source as usize).collect(),
            empty_is_null: req.empty_is_null,
            per_statement: (max_params(dialect) / req.mapping.len().max(1)).clamp(1, 500),
        }
    }

    fn row(&self, record: &Record) -> Vec<Option<String>> {
        self.sources
            .iter()
            .zip(&self.types)
            .map(|(&i, t)| {
                let v = record.values.get(i).cloned().flatten()?;
                if self.empty_is_null && v.is_empty() {
                    return None;
                }
                Some(normalize(self.dialect, t.as_deref(), v))
            })
            .collect()
    }

    fn insert(&self, rows: usize) -> String {
        let cols = self
            .columns
            .iter()
            .map(|c| nd_sql::ident(self.dialect, c))
            .collect::<Vec<_>>()
            .join(", ");
        let width = self.columns.len();
        if self.dialect == Dialect::Oracle {
            let into = (0..rows)
                .map(|r| {
                    let marks = (1..=width)
                        .map(|c| placeholder(self.dialect, r * width + c))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("INTO {} ({cols}) VALUES ({marks})", self.table)
                })
                .collect::<Vec<_>>()
                .join(" ");
            return format!("INSERT ALL {into} SELECT 1 FROM dual");
        }
        let values = (0..rows)
            .map(|r| {
                let marks = (1..=width)
                    .map(|c| placeholder(self.dialect, r * width + c))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({marks})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("INSERT INTO {} ({cols}) VALUES {values}", self.table)
    }
}

fn create_sql(dialect: Dialect, table: &str, columns: &[NewColumn]) -> String {
    let defs = columns
        .iter()
        .map(|c| {
            let null = if c.nullable { "" } else { " NOT NULL" };
            format!("{} {}{null}", nd_sql::ident(dialect, &c.name), c.type_name)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("CREATE TABLE {table} ({defs})")
}

fn plain_type(t: &str) -> bool {
    !t.trim().is_empty()
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || " _(),".contains(c))
}

pub(super) struct Inserter<'a> {
    conn: &'a dyn Connection,
    plan: Plan,
    on_error: OnRowError,
    pub(super) inserted: u64,
    skipped: u64,
    pub(super) faults: Vec<RowFault>,
    faults_cut: bool,
    pub(super) failed: bool,
    transactional: bool,
    pub(super) refused: Option<(u64, u64)>,
}

impl<'a> Inserter<'a> {
    pub(super) fn stopping(conn: &'a dyn Connection, plan: Plan) -> Self {
        Self {
            conn,
            plan,
            on_error: OnRowError::Stop,
            inserted: 0,
            skipped: 0,
            faults: Vec::new(),
            faults_cut: false,
            failed: false,
            transactional: true,
            refused: None,
        }
    }

    pub(super) fn batches(conn: &'a dyn Connection, plan: Plan) -> Self {
        Self {
            transactional: false,
            ..Self::stopping(conn, plan)
        }
    }

    async fn run(
        &mut self,
        create: Option<&str>,
        mut rx: mpsc::Receiver<Batch>,
        job: &super::Job,
        progress: &(dyn Fn(Progress) + Send + Sync),
    ) -> std::result::Result<(), DriverError> {
        let (conn, dialect) = (self.conn, self.plan.dialect);
        let implicit_commit = dialect == Dialect::Mysql;
        if let Some(sql) = create.filter(|_| implicit_commit) {
            quiet(conn, sql).await?;
        }
        if self.transactional {
            quiet(conn, nd_sql::begin(dialect)).await?;
        }
        if let Some(sql) = create.filter(|_| !implicit_commit) {
            quiet(conn, sql).await?;
        }
        let mut pace = Pace::new();
        while let Some(batch) = rx.recv().await {
            if job.stopped() || self.failed {
                break;
            }
            if let Batch::Records {
                records,
                done,
                total,
                unit,
            } = batch
            {
                self.rows(&records).await?;
                if pace.due() {
                    let rows = self.inserted + self.skipped;
                    progress(Progress {
                        done,
                        total,
                        unit,
                        rows,
                        bytes: 0,
                        stage: None,
                    });
                }
            }
        }
        Ok(())
    }

    fn fault(&mut self, line: u64, e: &DriverError) {
        self.skipped += 1;
        if self.faults.len() < MAX_FAULTS {
            self.faults.push(RowFault {
                line,
                message: e.to_string(),
            });
        } else {
            self.faults_cut = true;
        }
        if self.on_error == OnRowError::Stop {
            self.failed = true;
        }
    }

    async fn mark(&self, name: &str) -> std::result::Result<(), DriverError> {
        match self.plan.dialect {
            Dialect::Mssql => quiet(self.conn, &format!("SAVE TRANSACTION {name}")).await,
            Dialect::Duckdb | Dialect::Clickhouse => Ok(()),
            _ => quiet(self.conn, &format!("SAVEPOINT {name}")).await,
        }
    }

    async fn release(&self, name: &str) -> std::result::Result<(), DriverError> {
        match self.plan.dialect {
            Dialect::Mssql | Dialect::Duckdb | Dialect::Clickhouse => Ok(()),
            _ => quiet(self.conn, &format!("RELEASE SAVEPOINT {name}")).await,
        }
    }

    async fn back(&self, name: &str) -> std::result::Result<(), DriverError> {
        match self.plan.dialect {
            Dialect::Mssql => quiet(self.conn, &format!("ROLLBACK TRANSACTION {name}")).await,
            Dialect::Duckdb | Dialect::Clickhouse => Ok(()),
            _ => quiet(self.conn, &format!("ROLLBACK TO SAVEPOINT {name}")).await,
        }
    }

    pub(super) async fn rows(
        &mut self,
        records: &[Record],
    ) -> std::result::Result<(), DriverError> {
        for chunk in records.chunks(self.plan.per_statement) {
            if self.failed {
                return Ok(());
            }
            let params: Vec<Option<String>> = chunk.iter().flat_map(|r| self.plan.row(r)).collect();
            self.mark("nd_batch").await?;
            match collect(self.conn, &self.plan.insert(chunk.len()), Some(&params)).await {
                Ok(_) => {
                    self.release("nd_batch").await?;
                    self.inserted += chunk.len() as u64;
                }
                Err(DriverError::Cancelled) => return Err(DriverError::Cancelled),
                Err(e) if self.plan.dialect == Dialect::Duckdb && self.transactional => {
                    return self.probe(chunk, &e).await;
                }
                Err(e) if self.plan.dialect == Dialect::Clickhouse => {
                    let first = chunk[0].line;
                    let last = chunk.last().map_or(first, |r| r.line);
                    self.refused = Some((first, last));
                    self.fault(first, &e);
                    self.failed = true;
                    return Ok(());
                }
                Err(_) => {
                    self.back("nd_batch").await?;
                    self.release("nd_batch").await?;
                    self.one_by_one(chunk).await?;
                }
            }
        }
        Ok(())
    }

    async fn probe(
        &mut self,
        chunk: &[Record],
        batch: &DriverError,
    ) -> std::result::Result<(), DriverError> {
        let _ = quiet(self.conn, "ROLLBACK").await;
        quiet(self.conn, nd_sql::begin(self.plan.dialect)).await?;
        let sql = self.plan.insert(1);
        let mut found = None;
        for r in chunk {
            match collect(self.conn, &sql, Some(&self.plan.row(r))).await {
                Ok(_) => {}
                Err(DriverError::Cancelled) => return Err(DriverError::Cancelled),
                Err(e) => {
                    found = Some((r.line, e));
                    break;
                }
            }
        }
        let _ = quiet(self.conn, "ROLLBACK").await;
        let (line, e) = found.unwrap_or_else(|| (chunk[0].line, batch.clone()));
        self.fault(line, &e);
        self.failed = true;
        Ok(())
    }

    async fn one_by_one(&mut self, chunk: &[Record]) -> std::result::Result<(), DriverError> {
        let sql = self.plan.insert(1);
        for r in chunk {
            if self.failed {
                return Ok(());
            }
            self.mark("nd_row").await?;
            match collect(self.conn, &sql, Some(&self.plan.row(r))).await {
                Ok(_) => {
                    self.release("nd_row").await?;
                    self.inserted += 1;
                }
                Err(DriverError::Cancelled) => return Err(DriverError::Cancelled),
                Err(e) => {
                    self.back("nd_row").await?;
                    self.release("nd_row").await?;
                    self.fault(r.line, &e);
                }
            }
        }
        Ok(())
    }
}

impl Core {
    pub async fn export_data(
        &self,
        req: DataExport,
        progress: impl Fn(Progress) + Send + Sync,
    ) -> Result<DataExported> {
        let session = self.live(&req.session_id).await?;
        let dialect = session.conn.sql()?;
        let path = checked_path(&req.path, req.format)?;
        let sql = match &req.source {
            DataExportSource::Table { namespace, name } => {
                format!("SELECT * FROM {}", qualified(dialect, namespace, name))
            }
            DataExportSource::Query { sql } => single_read(sql, dialect)?,
        };
        self.authorize(&session, Access::Read)?;
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let side = self.dial_tool(&session).await?;
        let conn = side.conn.as_ref();
        slot.job.watch(conn.canceller());

        let full = Arc::new(AtomicBool::new(false));
        let written = Arc::new(AtomicU64::new(0));
        let (to_file, mut from_db) = mpsc::channel::<Chunk>(4);
        let writer = {
            let (path, full, written) = (path.clone(), full.clone(), written.clone());
            let (format, options) = (req.format, WriteOptions { header: req.header });
            tokio::task::spawn_blocking(move || -> nd_transfer::Result<()> {
                let mut sink = None;
                while let Some(chunk) = from_db.blocking_recv() {
                    match chunk {
                        Chunk::Columns(columns) if sink.is_none() => {
                            sink = Some(nd_transfer::create(&path, format, &columns, &options)?);
                        }
                        Chunk::Columns(_) | Chunk::Documents(_) | Chunk::Progress(_) => {}
                        Chunk::Rows(rows) => {
                            let Some(s) = sink.as_mut() else { continue };
                            let took = s.rows(&rows)?;
                            written.fetch_add(took as u64, Ordering::Relaxed);
                            if took < rows.len() {
                                full.store(true, Ordering::Release);
                                break;
                            }
                        }
                    }
                }
                match sink {
                    Some(s) => s.finish(),
                    None => Ok(()),
                }
            })
        };

        let (tx, mut rx) = mpsc::channel::<Chunk>(4);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let expected = req.expected_rows.map(|n| n.max(0.0) as u64);
        let forward = async {
            let mut pace = Pace::new();
            let mut seen = 0u64;
            while let Some(chunk) = rx.recv().await {
                if let Chunk::Rows(r) = &chunk {
                    seen += r.len() as u64;
                }
                if full.load(Ordering::Acquire) || to_file.send(chunk).await.is_err() {
                    conn.canceller().cancel().await;
                    break;
                }
                if pace.due() {
                    progress(Progress {
                        done: seen,
                        total: expected.map(|e| e.max(seen)),
                        unit: Unit::Rows,
                        rows: seen,
                        bytes: 0,
                        stage: None,
                    });
                }
            }
            drop(to_file);
            while rx.recv().await.is_some() {}
        };
        let (outcome, ()) = tokio::join!(conn.execute(&sql, tx), forward);
        let wrote = writer.await.map_err(|e| CoreError::Io(e.to_string()))?;
        let truncated = full.load(Ordering::Acquire);
        let cancelled = slot.job.stopped();
        let failure = match (outcome, wrote) {
            (_, Err(e)) => Some(CoreError::from(e)),
            (Err(DriverError::Cancelled), Ok(())) if truncated || cancelled => None,
            (Err(e), Ok(())) => Some(e.into()),
            (Ok(_), Ok(())) => None,
        };
        if failure.is_some() || cancelled {
            let _ = tokio::fs::remove_file(&path).await;
        }
        if let Some(e) = failure {
            return Err(e);
        }
        let bytes = tokio::fs::metadata(&path).await.map_or(0, |m| m.len());
        Ok(DataExported {
            rows: written.load(Ordering::Relaxed),
            bytes,
            elapsed_ms: ms(started),
            truncated,
            cancelled,
        })
    }

    pub async fn import_sample(
        &self,
        session_id: &str,
        source: DataImportSource,
    ) -> Result<DataImportSample> {
        let dialect = self.session(session_id)?.conn.sql()?;
        let path = checked_path(&source.path, source.format)?;
        let options = read_options(&source);
        tokio::task::spawn_blocking(move || -> Result<DataImportSample> {
            let (mut names, mut sheets, mut rows) = (Vec::new(), Vec::new(), Vec::new());
            let mut more = false;
            nd_transfer::read(&path, source.format, &options, 500, &mut |b| match b {
                Batch::Columns {
                    names: n,
                    sheets: s,
                } => {
                    names = n;
                    sheets = s;
                    true
                }
                Batch::Records { records, .. } => {
                    rows.extend(records.into_iter().map(|r| r.values));
                    if rows.len() > SAMPLE_ROWS {
                        more = true;
                        rows.truncate(SAMPLE_ROWS);
                    }
                    !more
                }
            })?;
            let columns = names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let values = rows.iter().map(|r| r.get(i).and_then(Option::as_deref));
                    let (inferred, longest) = nd_transfer::infer(values.clone());
                    SampleColumn {
                        name: name.clone(),
                        inferred,
                        type_name: nd_transfer::sql_type(dialect, inferred, longest),
                        nullable: values.into_iter().any(|v| v.is_none_or(str::is_empty)),
                    }
                })
                .collect();
            more |= rows.len() > PREVIEW_ROWS;
            rows.truncate(PREVIEW_ROWS);
            Ok(DataImportSample {
                columns,
                rows,
                sheets,
                more,
            })
        })
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?
    }

    pub async fn import_data(
        &self,
        req: DataImport,
        progress: impl Fn(Progress) + Send + Sync,
    ) -> Result<DataImported> {
        let session = self.live(&req.session_id).await?;
        let dialect = session.conn.sql()?;
        let path = checked_path(&req.source.path, req.source.format)?;
        let (namespace, name, created) = target_of(&req)?;
        self.authorize(
            &session,
            Access::Write {
                confirm: req.confirm.as_deref(),
            },
        )?;
        let table = qualified(dialect, namespace, name);
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let side = self.dial_tool(&session).await?;
        let conn = side.conn.as_ref();
        slot.job.watch(conn.canceller());
        let types = match created {
            Some(cols) => req
                .mapping
                .iter()
                .map(|m| {
                    cols.iter()
                        .find(|c| c.name == m.target)
                        .map(|c| c.type_name.clone())
                })
                .collect(),
            None => existing_types(conn, namespace, name, &req.mapping).await?,
        };
        let plan = Plan::of(dialect, table.clone(), &req, types);
        if session.guarded()
            && let Some(sql) = nd_sql::read_only(dialect, false)
        {
            quiet(conn, sql).await?;
        }
        let create = created.map(|cols| create_sql(dialect, &table, cols));

        let (tx, rx) = mpsc::channel::<Batch>(2);
        let reader = {
            let (path, format, options) =
                (path.clone(), req.source.format, read_options(&req.source));
            tokio::task::spawn_blocking(move || {
                nd_transfer::read(&path, format, &options, READ_BATCH, &mut |b| {
                    tx.blocking_send(b).is_ok()
                })
            })
        };
        let mut ins = Inserter {
            conn,
            plan,
            on_error: req.on_error,
            inserted: 0,
            skipped: 0,
            faults: Vec::new(),
            faults_cut: false,
            failed: false,
            transactional: !(dialect == Dialect::Duckdb && req.on_error == OnRowError::Skip),
            refused: None,
        };
        let ran = ins.run(create.as_deref(), rx, &slot.job, &progress).await;
        let read = reader.await.map_err(|e| CoreError::Io(e.to_string()))?;
        let cancelled = slot.job.stopped();
        let ok = ran.is_ok() && read.is_ok() && !cancelled && !ins.failed;
        let committed = finish(conn, ok, create.is_some(), &table, ins.transactional).await;
        let mut script = vec![nd_sql::begin(dialect).to_owned()];
        script.extend(create);
        script.push(format!("-- {} → {table}", file_name(&path)));
        script.push(if committed { "COMMIT" } else { "ROLLBACK" }.into());
        let error = match (&ran, &read) {
            (Err(DriverError::Cancelled), _) if cancelled => None,
            (Err(e), _) => Some(e.to_string()),
            (_, Err(e)) => Some(e.to_string()),
            _ => ins
                .faults
                .first()
                .filter(|_| ins.failed)
                .map(|f| format!("line {}: {}", f.line, f.message)),
        };
        self.audit(
            &session,
            &script.join(";\n"),
            committed.then_some(ins.inserted),
            error,
        );
        match (ran, read) {
            (Err(DriverError::Cancelled), _) if cancelled => {}
            (Err(e), _) => return Err(e.into()),
            (_, Err(e)) => return Err(e.into()),
            _ => {}
        }
        Ok(DataImported {
            inserted: if committed { ins.inserted } else { 0 },
            skipped: ins.skipped,
            faults: ins.faults,
            faults_cut: ins.faults_cut,
            committed,
            cancelled,
            elapsed_ms: ms(started),
        })
    }
}

async fn finish(
    conn: &dyn Connection,
    ok: bool,
    created: bool,
    table: &str,
    transactional: bool,
) -> bool {
    if ok && (!transactional || quiet(conn, "COMMIT").await.is_ok()) {
        return true;
    }
    let _ = quiet(conn, "ROLLBACK").await;
    if created && (conn.dialect() == Some(Dialect::Mysql) || !transactional) {
        let _ = quiet(conn, &format!("DROP TABLE IF EXISTS {table}")).await;
    }
    false
}

type Target<'a> = (&'a String, &'a String, Option<&'a Vec<NewColumn>>);

fn target_of(req: &DataImport) -> Result<Target<'_>> {
    let empty = || CoreError::from(ToolFault::Empty);
    if req.mapping.is_empty() {
        return Err(empty());
    }
    let (namespace, name, created) = match &req.target {
        DataImportTarget::Existing { namespace, name } => (namespace, name, None),
        DataImportTarget::Create {
            namespace,
            name,
            columns,
        } => {
            if columns.is_empty() || columns.iter().any(|c| !plain_type(&c.type_name)) {
                return Err(empty());
            }
            (namespace, name, Some(columns))
        }
    };
    if name.trim().is_empty() {
        return Err(empty());
    }
    Ok((namespace, name, created))
}

async fn existing_types(
    conn: &dyn Connection,
    namespace: &str,
    name: &str,
    mapping: &[ColumnMap],
) -> Result<Vec<Option<String>>> {
    let schema = conn.schema().await?;
    let rel = schema
        .namespaces
        .iter()
        .filter(|n| namespace.is_empty() || n.name == namespace)
        .flat_map(|n| &n.tables)
        .find(|t| t.name == name)
        .ok_or_else(|| CoreError::NotFound(name.to_owned()))?;
    Ok(mapping
        .iter()
        .map(|m| {
            rel.columns
                .iter()
                .find(|c| c.name == m.target)
                .map(|c| c.type_name.clone())
        })
        .collect())
}
