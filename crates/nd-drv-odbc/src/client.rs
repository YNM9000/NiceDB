use std::sync::{Arc, OnceLock};
use std::thread;

use nd_driver::{
    CatalogRows, Chunk, Column, Connection, DdlKind, Dependency, DriverError, ObjectRef, Outcome,
    Result, RowBatcher, Schema, ServerInfo, assemble_catalog, tidy,
};
use nd_sql::Dialect;
use odbc_api::buffers::{BufferDesc, ColumnarDynBuffer, TextRowSet};
use odbc_api::handles::{AsStatementRef, Statement as _};
use odbc_api::{ConnectionOptions, Cursor, Environment, IntoParameter, ResultSetMetadata};
use tokio::sync::{mpsc, oneshot};

use crate::cancel::Running;
use crate::db2;
use crate::statement::{
    Kind, kind, rejected, sized_type, take_window, tidy_message, version_line, version_sql,
};

type Rows = Vec<Vec<Option<String>>>;
type Odbc = odbc_api::Connection<'static>;

const BATCH: usize = 500;
const MAX_TEXT: usize = 64 * 1024;

pub(crate) trait Backend {
    fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<bool>,
    ) -> Result<Option<u64>>;
    fn autocommit(&mut self, on: bool) -> Result<()>;
    fn end(&mut self, commit: bool) -> Result<()>;
    fn catalog(&mut self) -> Result<CatalogRows>;
    fn product(&mut self) -> String;
    fn database(&mut self) -> String;
    fn quote(&mut self) -> Option<char> {
        None
    }
}

pub(crate) trait Stop: Send + Sync {
    fn stop(&self) -> bool;
}

impl Stop for Running {
    fn stop(&self) -> bool {
        self.cancel()
    }
}

fn env() -> Result<&'static Environment> {
    static ENV: OnceLock<std::result::Result<Environment, String>> = OnceLock::new();
    ENV.get_or_init(|| Environment::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| DriverError::Connect { message: e.clone() })
}

fn record_message(record: &odbc_api::handles::Record) -> String {
    let spelled = record.to_string();
    let text = spelled
        .split_once("Message: ")
        .map_or(spelled.as_str(), |(_, m)| m);
    tidy_message(text)
}

fn connect_err(e: &odbc_api::Error) -> DriverError {
    let message = match e {
        odbc_api::Error::Diagnostics { record, .. } => record_message(record),
        _ => e.to_string(),
    };
    DriverError::Connect { message }
}

fn map_err(e: odbc_api::Error) -> DriverError {
    match &e {
        odbc_api::Error::Diagnostics { record, .. } => DriverError::Query {
            message: record_message(record),
            code: Some(String::from_utf8_lossy(&record.state.0).into_owned()),
            detail: None,
            hint: None,
            offset: None,
        },
        _ => DriverError::query(e.to_string()),
    }
}

enum Job {
    Execute {
        sql: String,
        params: Vec<Option<String>>,
        out: mpsc::Sender<Chunk>,
        reply: oneshot::Sender<Result<Outcome>>,
    },
    Schema {
        reply: oneshot::Sender<Result<Schema>>,
    },
    Ddl {
        object: ObjectRef,
        reply: oneshot::Sender<Result<String>>,
    },
    Dependencies {
        object: ObjectRef,
        reply: oneshot::Sender<Result<Vec<Dependency>>>,
    },
}

pub struct OdbcConnection {
    jobs: std::sync::mpsc::Sender<Job>,
    server: ServerInfo,
    dialect: Dialect,
    stop: Arc<dyn Stop>,
}

struct Worker {
    be: Box<dyn Backend>,
    dialect: Dialect,
    own_schema: String,
    server_window: Option<bool>,
    in_transaction: bool,
    database: String,
}

struct OdbcBackend {
    conn: Odbc,
    running: Running,
}

pub async fn connect(text: String, dialect: Dialect) -> Result<Box<dyn Connection>> {
    let env = env()?;
    let running = Running::default();
    let shared = running.clone();
    spawn(dialect, Arc::new(running), move || {
        let conn = env
            .connect_with_connection_string(&text, ConnectionOptions::default())
            .map_err(|e| connect_err(&e))?;
        Ok(Box::new(OdbcBackend {
            conn,
            running: shared,
        }) as Box<dyn Backend>)
    })
    .await
}

pub(crate) async fn spawn(
    dialect: Dialect,
    stop: Arc<dyn Stop>,
    open: impl FnOnce() -> Result<Box<dyn Backend>> + Send + 'static,
) -> Result<Box<dyn Connection>> {
    let (ready_tx, ready_rx) = oneshot::channel::<Result<(ServerInfo, Dialect)>>();
    let (jobs, rx) = std::sync::mpsc::channel::<Job>();
    thread::Builder::new()
        .name("nd-odbc".into())
        .spawn(move || {
            let be = match open() {
                Ok(b) => b,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let mut w = Worker {
                be,
                dialect,
                own_schema: String::new(),
                server_window: None,
                in_transaction: false,
                database: String::new(),
            };
            if dialect.is_generic() && w.be.quote() == Some('`') {
                w.dialect = Dialect::AnsiBacktick;
            }
            let db2 = dialect == Dialect::Db2;
            let product = if db2 {
                "Db2".to_owned()
            } else {
                w.be.product()
            };
            let version = if db2 {
                w.first(db2::VERSION)
            } else {
                version_sql(&product)
                    .map(|q| version_line(&w.first(q)))
                    .unwrap_or_default()
            };
            if db2 {
                w.own_schema = w.first(db2::CURRENT_SCHEMA);
            }
            let database = w.be.database();
            w.database.clone_from(&database);
            let _ = ready_tx.send(Ok((
                ServerInfo {
                    product,
                    version,
                    database,
                },
                w.dialect,
            )));
            for job in rx {
                w.serve(job);
            }
        })
        .map_err(|e| DriverError::Io {
            message: e.to_string(),
        })?;
    let (server, dialect) = ready_rx.await.map_err(|_| gone())??;
    Ok(Box::new(OdbcConnection {
        jobs,
        server,
        dialect,
        stop,
    }))
}

fn gone() -> DriverError {
    DriverError::Io {
        message: "odbc worker exited".into(),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("\\x");
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn buffer_desc(ty: &odbc_api::DataType, shown: Option<std::num::NonZeroUsize>) -> BufferDesc {
    use odbc_api::DataType as T;
    match *ty {
        T::Binary { length } | T::Varbinary { length } => BufferDesc::Binary {
            max_bytes: length.map_or(MAX_TEXT, |n| n.get().min(MAX_TEXT)),
        },
        T::LongVarbinary { .. }
        | T::Other {
            data_type: odbc_api::sys::SqlDataType(-98),
            ..
        } => BufferDesc::Binary {
            max_bytes: MAX_TEXT,
        },
        _ => BufferDesc::Text {
            max_str_len: ty
                .utf8_len()
                .or(shown)
                .map_or(MAX_TEXT, std::num::NonZeroUsize::get)
                .clamp(64, MAX_TEXT),
        },
    }
}

impl Worker {
    fn serve(&mut self, job: Job) {
        match job {
            Job::Execute {
                sql,
                params,
                out,
                reply,
            } => {
                let _ = reply.send(self.execute(&sql, &params, out));
            }
            Job::Schema { reply } => {
                let _ = reply.send(self.schema());
            }
            Job::Ddl { object, reply } => {
                let _ = reply.send(self.ddl(&object));
            }
            Job::Dependencies { object, reply } => {
                let _ = reply.send(self.dependencies(&object));
            }
        }
    }

    fn first(&mut self, sql: &str) -> String {
        self.query(sql, &[])
            .ok()
            .and_then(|r| r.into_iter().next())
            .and_then(|r| r.into_iter().next().flatten())
            .unwrap_or_default()
    }

    fn query(&mut self, sql: &str, params: &[Option<String>]) -> Result<Rows> {
        let mut rows = Vec::new();
        self.run(sql, params, &mut |_| Ok(()), &mut |r| {
            rows.push(r);
            Ok(())
        })?;
        Ok(rows)
    }

    fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<()>,
    ) -> Result<Option<u64>> {
        let (bare, window) = if self.dialect.is_generic() && nd_sql::is_read(sql, self.dialect) {
            take_window(sql)
        } else {
            (sql, None)
        };
        let Some(window) = window else {
            return self
                .be
                .run(sql, params, head, &mut |r| row(r).map(|()| true));
        };
        if self.server_window == Some(true)
            || (self.server_window.is_none() && !self.in_transaction)
        {
            let mut headed = false;
            let tried = self.be.run(
                sql,
                params,
                &mut |c| {
                    headed = true;
                    head(c)
                },
                &mut |r| row(r).map(|()| true),
            );
            match tried {
                Ok(n) => {
                    self.server_window = Some(true);
                    return Ok(n);
                }
                Err(e) if self.server_window.is_some() || headed || !rejected(&e) => {
                    return Err(e);
                }
                Err(_) => {}
            }
        }
        let (mut skip, mut left) = (window.skip, window.take);
        let n = self.be.run(bare, params, head, &mut |r| {
            if skip > 0 {
                skip -= 1;
                return Ok(true);
            }
            left = left.saturating_sub(1);
            row(r)?;
            Ok(left > 0)
        })?;
        if !self.in_transaction {
            self.server_window = Some(false);
        }
        Ok(n)
    }

    fn execute(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let command = sql.split_whitespace().next().map(str::to_uppercase);
        match kind(sql) {
            Kind::Begin => {
                self.be.autocommit(false)?;
                self.in_transaction = true;
            }
            Kind::Commit | Kind::Rollback => {
                self.be.end(kind(sql) == Kind::Commit)?;
                self.be.autocommit(true)?;
                self.in_transaction = false;
            }
            Kind::Plain => {
                let sink = std::cell::RefCell::new(RowBatcher::new(out));
                let n = self.run(
                    sql,
                    params,
                    &mut |c| sink.borrow().columns_blocking(c),
                    &mut |r| sink.borrow_mut().push_blocking(r),
                )?;
                sink.borrow_mut().flush_blocking()?;
                return Ok(Outcome {
                    command,
                    rows_affected: n,
                });
            }
        }
        Ok(Outcome {
            command,
            rows_affected: None,
        })
    }

    fn schema(&mut self) -> Result<Schema> {
        let rows = if self.dialect == Dialect::Db2 {
            let mut answers = Vec::new();
            for sql in db2::catalog_sql() {
                answers.push(self.query(&sql, &[])?);
            }
            db2::catalog(answers)
        } else {
            self.be.catalog()?
        };
        let own = (!self.own_schema.is_empty()).then_some(self.own_schema.as_str());
        Ok(Schema {
            databases: Vec::new(),
            current_database: self.database.clone(),
            namespaces: assemble_catalog(&rows, own),
        })
    }

    fn text(&mut self, sql: &str, params: &[&str], o: &ObjectRef) -> Result<String> {
        let params: Vec<Option<String>> = params.iter().map(|p| Some((*p).to_owned())).collect();
        self.query(sql, &params)?
            .into_iter()
            .next()
            .and_then(|r| r.into_iter().next().flatten())
            .ok_or_else(|| o.not_found())
    }

    fn ddl(&mut self, o: &ObjectRef) -> Result<String> {
        let (ns, name) = (o.namespace.as_str(), o.name.as_str());
        if self.dialect != Dialect::Db2 && o.kind != DdlKind::Table {
            return Err(DriverError::query(
                "only tables are scripted over generic ODBC",
            ));
        }
        match o.kind {
            DdlKind::Table => {
                let s = self.schema()?;
                let t = s
                    .namespaces
                    .iter()
                    .filter(|n| n.name == ns)
                    .flat_map(|n| &n.tables)
                    .find(|t| t.name == name)
                    .ok_or_else(|| o.not_found())?;
                Ok(db2::create_table(self.dialect, ns, t))
            }
            DdlKind::View => self.text(db2::VIEW_TEXT, &[ns, name], o),
            DdlKind::Routine => {
                let specific = o.signature.as_deref().unwrap_or(name);
                self.text(db2::ROUTINE_TEXT, &[ns, specific, name], o)
            }
            DdlKind::Trigger => self.text(db2::TRIGGER_TEXT, &[ns, name], o),
            DdlKind::Sequence => {
                let p = [Some(ns.to_owned()), Some(name.to_owned())];
                let facts = self
                    .query(db2::SEQUENCE_FACTS, &p)?
                    .into_iter()
                    .next()
                    .ok_or_else(|| o.not_found())?;
                Ok(db2::sequence_ddl(ns, name, &facts))
            }
            DdlKind::Index => {
                let p = [Some(ns.to_owned()), Some(name.to_owned())];
                let facts = self
                    .query(db2::INDEX_FACTS, &p)?
                    .into_iter()
                    .next()
                    .ok_or_else(|| o.not_found())?;
                let cols: Vec<(String, String)> = self
                    .query(db2::INDEX_COLUMNS, &p)?
                    .into_iter()
                    .map(|r| {
                        let c = |i: usize| r.get(i).cloned().flatten().unwrap_or_default();
                        (c(0), c(1))
                    })
                    .collect();
                let f = |i: usize| facts.get(i).cloned().flatten().unwrap_or_default();
                Ok(db2::index_ddl(ns, name, &f(0), &f(1), &f(2), &cols))
            }
        }
    }

    fn dependencies(&mut self, o: &ObjectRef) -> Result<Vec<Dependency>> {
        if self.dialect != Dialect::Db2 {
            return Ok(Vec::new());
        }
        let mut p = Vec::new();
        for _ in 0..5 {
            p.push(Some(o.namespace.clone()));
            p.push(Some(o.name.clone()));
        }
        let rows = self.query(db2::DEPENDENCIES, &p)?;
        Ok(tidy(
            rows.iter()
                .filter_map(|r| Dependency::from_row(r))
                .collect(),
        ))
    }
}

impl Backend for OdbcBackend {
    fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<bool>,
    ) -> Result<Option<u64>> {
        let bound: Vec<_> = params
            .iter()
            .map(|p| p.as_deref().into_parameter())
            .collect();
        let mut stmt = self.conn.preallocate().map_err(map_err)?;
        self.running.set(Some(stmt.as_stmt_ref().as_sys()));
        let mut failed = None;
        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Some(mut cursor) = stmt.execute(sql, &bound[..]).map_err(map_err)? else {
                return Ok(stmt.row_count().map_err(map_err)?.map(|n| n as u64));
            };
            let names: Vec<String> = cursor
                .column_names()
                .map_err(map_err)?
                .collect::<std::result::Result<_, _>>()
                .map_err(map_err)?;
            let mut descs = Vec::with_capacity(names.len());
            let mut columns = Vec::with_capacity(names.len());
            for (i, name) in names.into_iter().enumerate() {
                let n = u16::try_from(i + 1).unwrap_or(u16::MAX);
                let ty = cursor.col_data_type(n).map_err(map_err)?;
                let shown = cursor.col_display_size(n).ok().flatten();
                descs.push(buffer_desc(&ty, shown));
                columns.push(Column {
                    name,
                    type_name: Some(type_label(&ty)),
                });
            }
            head(columns)?;
            let binary: Vec<bool> = descs
                .iter()
                .map(|d| matches!(d, BufferDesc::Binary { .. }))
                .collect();
            let buffers = ColumnarDynBuffer::try_from_descs(BATCH, descs).map_err(map_err)?;
            let mut rows = cursor.bind_buffer(buffers).map_err(map_err)?;
            'fetch: while let Some(batch) = rows.fetch().map_err(|e| {
                let e = map_err(e);
                failed = Some(e.clone());
                e
            })? {
                for r in 0..batch.num_rows() {
                    let cells = binary
                        .iter()
                        .enumerate()
                        .map(|(c, bin)| {
                            if *bin {
                                batch.column(c).as_binary().and_then(|v| v.get(r)).map(hex)
                            } else {
                                batch
                                    .column(c)
                                    .as_text()
                                    .and_then(|v| v.get(r))
                                    .map(|b| String::from_utf8_lossy(b).into_owned())
                            }
                        })
                        .collect();
                    if !row(cells)? {
                        break 'fetch;
                    }
                }
            }
            Ok(None)
        }));
        self.running.set(None);
        done.unwrap_or_else(|_| {
            Err(failed.unwrap_or_else(|| {
                DriverError::query("the ODBC driver failed to close its cursor")
            }))
        })
    }

    fn autocommit(&mut self, on: bool) -> Result<()> {
        self.conn.set_autocommit(on).map_err(map_err)
    }

    fn end(&mut self, commit: bool) -> Result<()> {
        if commit {
            self.conn.commit().map_err(map_err)
        } else {
            self.conn.rollback().map_err(map_err)
        }
    }

    fn catalog(&mut self) -> Result<CatalogRows> {
        let mut stmt = self.conn.preallocate().map_err(map_err)?;
        let tables = read(Some(
            stmt.tables_cursor("", "%", "%", "TABLE,VIEW")
                .map_err(map_err)?,
        ))?;
        let mut rows = CatalogRows::default();
        let cell = |r: &Vec<Option<String>>, i: usize| r.get(i).cloned().flatten();
        let by_catalog = tables
            .iter()
            .all(|t| cell(t, 1).is_none_or(|s| s.is_empty()));
        let namespace = |r: &Vec<Option<String>>, catalog: usize| {
            cell(r, if by_catalog { catalog } else { catalog + 1 }).unwrap_or_default()
        };
        for t in &tables {
            let (schema, name) = (namespace(t, 0), cell(t, 2).unwrap_or_default());
            let (cat, sch) = if by_catalog {
                (schema.as_str(), "")
            } else {
                ("", schema.as_str())
            };
            let kind = if cell(t, 3).as_deref() == Some("VIEW") {
                "view"
            } else {
                "table"
            };
            rows.relations.push(vec![
                Some(schema.clone()),
                Some(name.clone()),
                Some(kind.into()),
                None,
            ]);
            for c in read(Some(
                stmt.columns_cursor(cat, sch, &name, "%").map_err(map_err)?,
            ))? {
                let nullable = if cell(&c, 10).as_deref() == Some("0") {
                    "N"
                } else {
                    "Y"
                };
                let ty = sized_type(
                    &cell(&c, 5).unwrap_or_default(),
                    cell(&c, 6).as_deref(),
                    cell(&c, 8).as_deref(),
                );
                rows.columns.push(vec![
                    Some(schema.clone()),
                    Some(name.clone()),
                    cell(&c, 3),
                    Some(ty),
                    Some(nullable.into()),
                ]);
            }
            if kind == "view" {
                continue;
            }
            let (pk_cat, pk_sch) = if by_catalog {
                (Some(cat), None)
            } else {
                (None, Some(sch))
            };
            for k in read(stmt.primary_keys_cursor(pk_cat, pk_sch, &name).ok())? {
                rows.indexes.push(vec![
                    Some(schema.clone()),
                    Some(name.clone()),
                    cell(&k, 5).or_else(|| Some("PRIMARY".into())),
                    cell(&k, 3),
                    Some("Y".into()),
                    Some("Y".into()),
                ]);
            }
            let fks = stmt.foreign_keys_cursor("", "", "", cat, sch, &name).ok();
            for f in read(fks)? {
                rows.foreign_keys.push(vec![
                    Some(schema.clone()),
                    Some(name.clone()),
                    cell(&f, 11).or_else(|| Some("FK".into())),
                    Some(namespace(&f, 0)),
                    cell(&f, 2),
                    cell(&f, 7),
                    cell(&f, 3),
                    Some(rule(cell(&f, 10)).into()),
                    Some(rule(cell(&f, 9)).into()),
                ]);
            }
        }
        Ok(rows)
    }

    fn product(&mut self) -> String {
        self.conn
            .database_management_system_name()
            .unwrap_or_else(|_| "ODBC".into())
    }

    fn database(&mut self) -> String {
        self.conn.current_catalog().unwrap_or_default()
    }

    fn quote(&mut self) -> Option<char> {
        self.conn.identifier_quote_char().ok().flatten()
    }
}

fn read<C: Cursor>(cursor: Option<C>) -> Result<Rows> {
    let Some(mut c) = cursor else {
        return Ok(Vec::new());
    };
    let mut buffers = TextRowSet::for_cursor(BATCH, &mut c, Some(4096)).map_err(map_err)?;
    let mut rows = c.bind_buffer(&mut buffers).map_err(map_err)?;
    let mut out = Vec::new();
    while let Some(b) = rows.fetch().map_err(map_err)? {
        for r in 0..b.num_rows() {
            out.push(
                (0..b.num_cols())
                    .map(|c| b.at(c, r).map(|x| String::from_utf8_lossy(x).into_owned()))
                    .collect(),
            );
        }
    }
    Ok(out)
}

fn rule(v: Option<String>) -> &'static str {
    match v.as_deref() {
        Some("0") => "CASCADE",
        Some("1") => "RESTRICT",
        Some("2") => "SET NULL",
        Some("4") => "SET DEFAULT",
        _ => "NO ACTION",
    }
}

fn type_label(ty: &odbc_api::DataType) -> String {
    use odbc_api::DataType as T;
    let sized = |name: &str, n: Option<std::num::NonZeroUsize>| {
        n.map_or_else(|| name.to_owned(), |n| format!("{name}({n})"))
    };
    match *ty {
        T::Char { length } | T::WChar { length } => sized("CHAR", length),
        T::Varchar { length } | T::WVarchar { length } => sized("VARCHAR", length),
        T::LongVarchar { .. } | T::WLongVarchar { .. } => "TEXT".into(),
        T::Numeric { precision, scale } | T::Decimal { precision, scale } => {
            format!("DECIMAL({precision},{scale})")
        }
        T::Integer => "INTEGER".into(),
        T::SmallInt => "SMALLINT".into(),
        T::TinyInt => "TINYINT".into(),
        T::BigInt => "BIGINT".into(),
        T::Float { .. } | T::Double => "DOUBLE".into(),
        T::Real => "REAL".into(),
        T::Bit => "BOOLEAN".into(),
        T::Date => "DATE".into(),
        T::Time { .. } => "TIME".into(),
        T::Timestamp { .. } => "TIMESTAMP".into(),
        T::Binary { length } => sized("BINARY", length),
        T::Varbinary { length } => sized("VARBINARY", length),
        T::LongVarbinary { .. } => "BLOB".into(),
        T::Other { data_type, .. } => format!("TYPE {}", data_type.0),
        T::Unknown => "UNKNOWN".into(),
    }
}

struct Canceller(Arc<dyn Stop>);

#[async_trait::async_trait]
impl nd_driver::Canceller for Canceller {
    async fn cancel(&self) {
        let stop = self.0.clone();
        let _ = tokio::task::spawn_blocking(move || stop.stop()).await;
    }
}

impl OdbcConnection {
    async fn ask<T>(&self, job: impl FnOnce(oneshot::Sender<Result<T>>) -> Job) -> Result<T> {
        let (reply, rx) = oneshot::channel();
        self.jobs.send(job(reply)).map_err(|_| gone())?;
        rx.await.map_err(|_| gone())?
    }
}

#[async_trait::async_trait]
impl Connection for OdbcConnection {
    fn capabilities(&self) -> nd_driver::Capabilities {
        crate::capabilities(self.dialect)
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn nd_driver::Canceller> {
        Arc::new(Canceller(self.stop.clone()))
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        self.execute_bound(sql, &[], out).await
    }

    async fn execute_bound(
        &self,
        sql: &str,
        params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        self.ask(|reply| Job::Execute {
            sql: sql.to_owned(),
            params: params.to_vec(),
            out,
            reply,
        })
        .await
    }

    async fn schema(&self) -> Result<Schema> {
        self.ask(|reply| Job::Schema { reply }).await
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        self.ask(|reply| Job::Ddl {
            object: object.clone(),
            reply,
        })
        .await
    }

    async fn dependencies(&self, object: &ObjectRef) -> Result<Vec<Dependency>> {
        self.ask(|reply| Job::Dependencies {
            object: object.clone(),
            reply,
        })
        .await
    }

    async fn ping(&self) -> Result<()> {
        let sql = if self.dialect == Dialect::Db2 {
            "SELECT 1 FROM SYSIBM.SYSDUMMY1"
        } else {
            "SELECT 1"
        };
        let (tx, mut rx) = mpsc::channel(4);
        let drain = async { while rx.recv().await.is_some() {} };
        let (done, ()) = tokio::join!(self.execute(sql, tx), drain);
        done.map(|_| ())
    }
}
