use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use nd_driver::{Dialect, KeyFile, LibsqlTarget, PrivateDir, SqliteTarget, Target, TlsMode};
use nd_transfer::Unit;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{ChildStderr, Command};
use zeroize::Zeroizing;

use super::{Job, Pace, Progress, ToolFault, ms, tail};
use crate::guard::{Access, collect};
use crate::session::Session;
use crate::{Core, CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BackupFormat {
    Sql,
    Archive,
    File,
    Folder,
    Bak,
    Zip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolFamily {
    Postgres,
    Mysql,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct BackupTool {
    pub name: String,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[allow(clippy::struct_excessive_bools)]
pub struct ToolCheck {
    pub dump: Option<BackupTool>,
    pub restore: Vec<BackupTool>,
    pub formats: Vec<BackupFormat>,
    pub builtin: bool,
    pub supported: bool,
    pub server_side: bool,
    pub can_restore: bool,
    pub schema_only: bool,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct BackupRequest {
    pub job_id: String,
    pub session_id: String,
    pub path: String,
    pub format: BackupFormat,
    pub schema_only: bool,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct RestoreRequest {
    pub job_id: String,
    pub session_id: String,
    pub path: String,
    pub confirm: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Backed {
    #[specta(type = f64)]
    pub bytes: u64,
    #[specta(type = f64)]
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub notes: Option<String>,
}

impl Backed {
    fn written(bytes: u64, elapsed_ms: u64) -> Self {
        Self {
            bytes,
            elapsed_ms,
            cancelled: false,
            notes: None,
        }
    }
}

const EXTENSIONS: &[&str] = &[
    "sql", "dump", "backup", "sqlite", "sqlite3", "db", "bak", "zip",
];
const PG_MAGIC: &[u8] = b"PGDMP";
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn extra_dirs(family: ToolFamily) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let globbed = |parent: &str, prefix: &str, tail: &str| -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(parent)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
            .map(|e| e.path().join(tail))
            .collect();
        found.sort();
        found.reverse();
        found
    };
    if family == ToolFamily::Postgres {
        dirs.extend(globbed("/usr/lib/postgresql", "", "bin"));
        dirs.extend(globbed("/usr", "pgsql-", "bin"));
        dirs.extend(globbed("/opt/homebrew/opt", "postgresql", "bin"));
        dirs.extend(globbed("/usr/local/opt", "postgresql", "bin"));
        dirs.push("/opt/homebrew/opt/libpq/bin".into());
        dirs.push("/usr/local/opt/libpq/bin".into());
        dirs.push("/Applications/Postgres.app/Contents/Versions/latest/bin".into());
        dirs.extend(globbed("C:\\Program Files\\PostgreSQL", "", "bin"));
    } else {
        dirs.extend(globbed("/opt/homebrew/opt", "mysql", "bin"));
        dirs.extend(globbed("/opt/homebrew/opt", "mariadb", "bin"));
        dirs.extend(globbed("C:\\Program Files\\MySQL", "MySQL", "bin"));
        dirs.extend(globbed("C:\\Program Files", "MariaDB", "bin"));
    }
    dirs
}

fn find(name: &str, family: ToolFamily, chosen: Option<&Path>) -> Option<PathBuf> {
    let file = exe(name);
    let path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    chosen
        .map(Path::to_path_buf)
        .into_iter()
        .chain(path)
        .chain(extra_dirs(family))
        .map(|d| d.join(&file))
        .find(|p| p.is_file())
}

async fn version(path: &Path) -> Option<String> {
    let out = Command::new(path).arg("--version").output().await.ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

async fn tool(name: &str, family: ToolFamily, chosen: Option<&Path>) -> Option<BackupTool> {
    let path = find(name, family, chosen)?;
    Some(BackupTool {
        name: name.to_owned(),
        version: version(&path).await,
        path: Some(path.to_string_lossy().into_owned()),
    })
}

fn mysql_names(mariadb: bool) -> ([&'static str; 2], [&'static str; 2]) {
    if mariadb {
        (["mariadb-dump", "mysqldump"], ["mariadb", "mysql"])
    } else {
        (["mysqldump", "mariadb-dump"], ["mysql", "mariadb"])
    }
}

async fn first_of(
    names: [&str; 2],
    family: ToolFamily,
    chosen: Option<&Path>,
) -> Option<BackupTool> {
    for n in names {
        if let Some(t) = tool(n, family, chosen).await {
            return Some(t);
        }
    }
    None
}

fn checked(path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    let ok = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)));
    if ok {
        Ok(p)
    } else {
        Err(ToolFault::WrongExtension.into())
    }
}

fn endpoint(session: &Session, host: &str, port: u16) -> (String, u16) {
    session.tunnel.as_ref().map_or_else(
        || (host.to_owned(), port),
        |t| (t.local.ip().to_string(), t.local.port()),
    )
}

fn pg_env(cmd: &mut Command, tls: TlsMode, ca: Option<&str>) {
    let mode = match tls {
        TlsMode::Disable => "disable",
        TlsMode::Verify => "verify-full",
        TlsMode::Insecure => "require",
    };
    cmd.env("PGSSLMODE", mode).env("PGCONNECT_TIMEOUT", "10");
    if let Some(ca) = ca {
        cmd.env("PGSSLROOTCERT", ca);
    } else if tls == TlsMode::Verify {
        cmd.env("PGSSLROOTCERT", "system");
    }
}

fn pgpass(password: &str) -> Option<Zeroizing<String>> {
    if password.contains(['\n', '\r']) {
        return None;
    }
    let mut line = Zeroizing::new(String::with_capacity(password.len() * 2 + 10));
    line.push_str("*:*:*:*:");
    for c in password.chars() {
        if matches!(c, '\\' | ':') {
            line.push('\\');
        }
        line.push(c);
    }
    line.push('\n');
    Some(line)
}

fn mysql_options(password: &str) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(password.len() * 2 + 24));
    out.push_str("[client]\npassword=\"");
    for c in password.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push_str("\"\n");
    out
}

fn private_file(name: &str, body: &str) -> Result<(PrivateDir, PathBuf)> {
    let io = |e: std::io::Error| CoreError::Io(e.to_string());
    let dir = PrivateDir::new().map_err(io)?;
    let path = dir.write(name, &[body.as_bytes()]).map_err(io)?;
    Ok((dir, path))
}

fn mysql_tls(cmd: &mut Command, tls: TlsMode, ca: Option<&str>, maria_client: bool) {
    match (tls, maria_client) {
        (TlsMode::Disable, true) => cmd.arg("--skip-ssl"),
        (TlsMode::Disable, false) => cmd.arg("--ssl-mode=DISABLED"),
        (TlsMode::Insecure, true) => cmd.args(["--ssl", "--skip-ssl-verify-server-cert"]),
        (TlsMode::Insecure, false) => cmd.arg("--ssl-mode=REQUIRED"),
        (TlsMode::Verify, true) => cmd.args(["--ssl", "--ssl-verify-server-cert"]),
        (TlsMode::Verify, false) => cmd.arg("--ssl-mode=VERIFY_IDENTITY"),
    };
    if let Some(ca) = ca {
        cmd.arg(format!("--ssl-ca={ca}"));
    }
}

fn table_line(line: &str) -> bool {
    line.contains("dumping contents of table")
        || line.contains("Retrieving table structure for table")
}

struct Run<'a> {
    job: &'a Job,
    progress: &'a (dyn Fn(Progress) + Send + Sync),
    tool: String,
}

struct Meter<'a> {
    progress: &'a (dyn Fn(Progress) + Send + Sync),
    bytes: AtomicU64,
    tables: AtomicU64,
    size: Option<u64>,
    total_tables: Option<u64>,
}

impl Meter<'_> {
    fn report(&self, pace: &mut Pace) {
        if !pace.due() {
            return;
        }
        let bytes = self.bytes.load(Ordering::Relaxed);
        let (done, total, unit) = match self.size {
            Some(size) => (bytes, Some(size), Unit::Bytes),
            None => (
                self.tables.load(Ordering::Relaxed),
                self.total_tables,
                Unit::Tables,
            ),
        };
        (self.progress)(Progress {
            done,
            total,
            unit,
            rows: 0,
            bytes,
            stage: None,
        });
    }
}

async fn pump(
    mut from: impl AsyncRead + Unpin,
    mut to: impl AsyncWrite + Unpin,
    meter: &Meter<'_>,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 256 * 1024];
    let mut pace = Pace::new();
    loop {
        let n = from.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        to.write_all(&buf[..n]).await?;
        meter.bytes.fetch_add(n as u64, Ordering::Relaxed);
        meter.report(&mut pace);
    }
    to.shutdown().await
}

async fn read_errors(stderr: Option<ChildStderr>, meter: &Meter<'_>) -> String {
    let mut text = String::new();
    let Some(stderr) = stderr else { return text };
    let mut lines = BufReader::new(stderr).lines();
    let mut pace = Pace::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if table_line(&line) {
            meter.tables.fetch_add(1, Ordering::Relaxed);
            meter.report(&mut pace);
        } else if text.len() < 64 * 1024 {
            text.push_str(&line);
            text.push('\n');
        }
    }
    text
}

fn piped(on: bool) -> Stdio {
    if on { Stdio::piped() } else { Stdio::null() }
}

impl Run<'_> {
    async fn go(
        &self,
        mut cmd: Command,
        out: Option<&Path>,
        input: Option<&Path>,
        tables: Option<u64>,
    ) -> Result<(bool, String)> {
        let io = |e: std::io::Error| CoreError::Io(e.to_string());
        cmd.stdin(piped(input.is_some()))
            .stdout(piped(out.is_some()))
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ToolFault::Missing {
                tool: self.tool.clone(),
            }
            .into(),
            _ => io(e),
        })?;
        let size = match input {
            Some(p) => Some(tokio::fs::metadata(p).await.map_err(io)?.len()),
            None => None,
        };
        let meter = Meter {
            progress: self.progress,
            bytes: AtomicU64::new(0),
            tables: AtomicU64::new(0),
            size,
            total_tables: tables,
        };
        let target = match out {
            Some(p) => Some(tokio::fs::File::create(p).await.map_err(io)?),
            None => None,
        };
        let source = match input {
            Some(p) => Some(tokio::fs::File::open(p).await.map_err(io)?),
            None => None,
        };
        let (stdout, stdin) = (child.stdout.take(), child.stdin.take());
        let copy_out = async {
            match (stdout, target) {
                (Some(from), Some(to)) => pump(from, to, &meter).await,
                _ => Ok(()),
            }
        };
        let copy_in = async {
            if let (Some(from), Some(to)) = (source, stdin) {
                let _ = pump(from, to, &meter).await;
            }
        };
        let work = async {
            let (text, wrote, ()) =
                tokio::join!(read_errors(child.stderr.take(), &meter), copy_out, copy_in);
            (text, wrote, child.wait().await)
        };
        tokio::select! {
            (text, wrote, status) = work => {
                wrote.map_err(io)?;
                if !status.map_err(io)?.success() {
                    return Err(ToolFault::Failed { tool: self.tool.clone(), message: tail(&text, 8) }.into());
                }
                Ok((false, text))
            }
            () = self.job.cancelled() => Ok((true, String::new())),
        }
    }
}

async fn sqlite_job(
    from: PathBuf,
    to: PathBuf,
    job: Arc<Job>,
    progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<bool> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::task::spawn_blocking(move || {
        sqlite_copy(&from, &to, &job, &|p| {
            let _ = tx.send(p);
        })
    });
    while let Some(p) = rx.recv().await {
        progress(p);
    }
    task.await.map_err(|e| CoreError::Io(e.to_string()))?
}

fn sqlite_copy(
    from: &Path,
    to: &Path,
    job: &Job,
    progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<bool> {
    use rusqlite::backup::{Backup, StepResult};
    let fail = |e: rusqlite::Error| CoreError::Io(e.to_string());
    let src =
        rusqlite::Connection::open_with_flags(from, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(fail)?;
    let mut dst = rusqlite::Connection::open(to).map_err(fail)?;
    let backup = Backup::new(&src, &mut dst).map_err(fail)?;
    let mut pace = Pace::new();
    loop {
        if job.stopped() {
            return Ok(true);
        }
        let step = backup.step(512).map_err(fail)?;
        let p = backup.progress();
        if pace.due() || step == StepResult::Done {
            let total = u64::try_from(p.pagecount).unwrap_or(0);
            let left = u64::try_from(p.remaining).unwrap_or(0);
            progress(Progress {
                done: total - left.min(total),
                total: Some(total),
                unit: Unit::Pages,
                rows: 0,
                bytes: 0,
                stage: None,
            });
        }
        match step {
            StepResult::Done => return Ok(false),
            StepResult::Busy | StepResult::Locked => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {}
        }
    }
}

async fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let mut total = 0;
    let mut pending = vec![path.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
            continue;
        };
        while let Ok(Some(e)) = entries.next_entry().await {
            match e.metadata().await {
                Ok(m) if m.is_dir() => pending.push(e.path()),
                Ok(m) => total += m.len(),
                Err(_) => {}
            }
        }
    }
    total
}

fn sql_text(text: &str) -> String {
    nd_sql::literal(Dialect::Duckdb, Some(text))
}

async fn head(path: &Path, n: usize) -> Result<Vec<u8>> {
    let mut f = tokio::fs::File::open(path)
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?;
    let mut buf = vec![0u8; n];
    let read = f
        .read(&mut buf)
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?;
    buf.truncate(read);
    Ok(buf)
}

impl Core {
    fn tool_dirs(&self) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
        let dirs = Self::lock(&self.tool_dirs)?;
        Ok((
            dirs.get(&ToolFamily::Postgres).cloned(),
            dirs.get(&ToolFamily::Mysql).cloned(),
        ))
    }

    pub fn set_tool_dir(&self, family: ToolFamily, dir: Option<String>) -> Result<()> {
        let mut dirs = Self::lock(&self.tool_dirs)?;
        match dir.filter(|d| !d.trim().is_empty()) {
            Some(d) => dirs.insert(family, PathBuf::from(d)),
            None => dirs.remove(&family),
        };
        Ok(())
    }

    fn profile_of(&self, session: &Session) -> Result<nd_store::ConnectionProfile> {
        let mut profile = self
            .store(|s| s.connection(&session.connection_id))?
            .ok_or_else(|| CoreError::NotFound(session.connection_id.clone()))?;
        profile.target = session.target.clone();
        Ok(profile)
    }

    pub async fn backup_tools(&self, session_id: &str) -> Result<ToolCheck> {
        let session = self.live(session_id).await?;
        let (pg, my) = self.tool_dirs()?;
        let mariadb = session.conn.server().product.contains("MariaDB");
        Ok(match self.profile_of(&session)?.target {
            Target::Postgres(_) => {
                let mut restore = Vec::new();
                restore.extend(tool("pg_restore", ToolFamily::Postgres, pg.as_deref()).await);
                restore.extend(tool("psql", ToolFamily::Postgres, pg.as_deref()).await);
                ToolCheck {
                    dump: tool("pg_dump", ToolFamily::Postgres, pg.as_deref()).await,
                    restore,
                    formats: vec![BackupFormat::Archive, BackupFormat::Sql],
                    builtin: false,
                    supported: true,
                    server_side: false,
                    can_restore: true,
                    schema_only: true,
                }
            }
            Target::Mysql(_) => {
                let (dump, restore) = mysql_names(mariadb);
                ToolCheck {
                    dump: first_of(dump, ToolFamily::Mysql, my.as_deref()).await,
                    restore: first_of(restore, ToolFamily::Mysql, my.as_deref())
                        .await
                        .into_iter()
                        .collect(),
                    formats: vec![BackupFormat::Sql],
                    builtin: false,
                    supported: true,
                    server_side: false,
                    can_restore: true,
                    schema_only: true,
                }
            }
            Target::Sqlite(_) | Target::Libsql(LibsqlTarget::File { .. }) => {
                Self::builtin_check(vec![BackupFormat::File], true)
            }
            Target::Duckdb(_) => Self::builtin_check(vec![BackupFormat::Folder], true),
            Target::Sqlserver(_) => ToolCheck {
                formats: vec![BackupFormat::Bak],
                ..Self::server_check()
            },
            Target::Clickhouse(_) => ToolCheck {
                formats: vec![BackupFormat::Zip],
                ..Self::server_check()
            },
            Target::Cockroach(_)
            | Target::Libsql(LibsqlTarget::Remote { .. })
            | Target::Mongodb(_)
            | Target::Redis(_)
            | Target::Cassandra(_)
            | Target::Dynamodb(_)
            | Target::Elasticsearch(_)
            | Target::Firebird(_)
            | Target::Oracle(_)
            | Target::Db2(_)
            | Target::Hana(_)
            | Target::Odbc(_) => Self::builtin_check(Vec::new(), false),
        })
    }

    fn server_check() -> ToolCheck {
        ToolCheck {
            dump: None,
            restore: Vec::new(),
            formats: Vec::new(),
            builtin: true,
            supported: true,
            server_side: true,
            can_restore: false,
            schema_only: false,
        }
    }

    fn builtin_check(formats: Vec<BackupFormat>, supported: bool) -> ToolCheck {
        ToolCheck {
            dump: None,
            restore: Vec::new(),
            formats,
            builtin: true,
            supported,
            server_side: false,
            can_restore: supported,
            schema_only: false,
        }
    }

    async fn table_count(&self, session: &Session) -> Option<u64> {
        let sql = match session.conn.dialect()? {
            nd_driver::Dialect::Postgres => {
                "SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relkind IN ('r', 'p') AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg_toast%'"
            }
            _ => {
                "SELECT count(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'"
            }
        };
        let side = self.side(session).await.ok()?;
        let (rows, _) = collect(side.conn().ok()?, sql, None).await.ok()?;
        rows.first()?.first()?.as_deref()?.parse().ok()
    }

    #[allow(clippy::too_many_lines)]
    pub async fn backup(
        &self,
        req: BackupRequest,
        progress: impl Fn(Progress) + Send + Sync,
    ) -> Result<Backed> {
        let session = self.live(&req.session_id).await?;
        self.authorize(&session, Access::Read)?;
        let profile = self.profile_of(&session)?;
        let path = match &profile.target {
            Target::Duckdb(_) => PathBuf::from(&req.path),
            _ => checked(&req.path)?,
        };
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let password = self.password_of(&profile.id).await?;
        let key = self.key_copy(&profile).await?;
        let (pg, my) = self.tool_dirs()?;
        let run = |tool: String| Run {
            job: &slot.job,
            progress: &progress,
            tool,
        };
        let (cancelled, notes) = match &profile.target {
            Target::Sqlite(SqliteTarget { path: from, .. })
            | Target::Libsql(LibsqlTarget::File { path: from, .. }) => {
                let job = slot.job.clone();
                (
                    sqlite_job(PathBuf::from(from), path.clone(), job, &progress).await?,
                    String::new(),
                )
            }
            Target::Duckdb(_) => {
                self.duckdb_export(&session, &slot.job, &path, &progress)
                    .await?
            }
            Target::Sqlserver(_) | Target::Clickhouse(_) => {
                let bytes = self.server_backup(&session, &slot.job, &req.path).await?;
                let elapsed_ms = ms(started);
                return Ok(Backed::written(bytes, elapsed_ms));
            }
            Target::Cockroach(_)
            | Target::Libsql(LibsqlTarget::Remote { .. })
            | Target::Mongodb(_)
            | Target::Redis(_)
            | Target::Cassandra(_)
            | Target::Dynamodb(_)
            | Target::Elasticsearch(_)
            | Target::Firebird(_)
            | Target::Oracle(_)
            | Target::Db2(_)
            | Target::Hana(_)
            | Target::Odbc(_) => {
                return Err(ToolFault::Unsupported.into());
            }
            Target::Postgres(t) => {
                let reach = Reach::of_pg(&session, t, &password).key(key.as_ref());
                let (mut cmd, _pass) = pg_command(pg_program("pg_dump", pg.as_deref())?, &reach)?;
                cmd.arg("--verbose");
                cmd.arg(if req.format == BackupFormat::Archive {
                    "--format=custom"
                } else {
                    "--format=plain"
                });
                if req.schema_only {
                    cmd.arg("--schema-only");
                }
                let tables = self.table_count(&session).await;
                run("pg_dump".into())
                    .go(cmd, Some(&path), None, tables)
                    .await?
            }
            Target::Mysql(t) => {
                let (names, _) = mysql_names(mariadb(&session));
                let found = mysql_program(names, my.as_deref()).await?;
                let reach = Reach::of_mysql(&session, t, &password).key(key.as_ref());
                let (mut cmd, _pass) = mysql_command(&found, &reach)?;
                cmd.args([
                    "--verbose",
                    "--single-transaction",
                    "--routines",
                    "--triggers",
                ]);
                if req.schema_only {
                    cmd.arg("--no-data");
                }
                cmd.arg(&t.database);
                let tables = self.table_count(&session).await;
                run(found.name).go(cmd, Some(&path), None, tables).await?
            }
        };
        if cancelled {
            let _ = tokio::fs::remove_file(&path).await;
            let _ = tokio::fs::remove_dir_all(&path).await;
        }
        Ok(Backed {
            bytes: if cancelled { 0 } else { tree_size(&path).await },
            elapsed_ms: ms(started),
            cancelled,
            notes: Some(tail(&notes, 6))
                .filter(|n| !n.is_empty() && n.to_ascii_lowercase().contains("warning")),
        })
    }

    async fn duckdb_export(
        &self,
        session: &Session,
        job: &Job,
        dir: &Path,
        progress: &(dyn Fn(Progress) + Send + Sync),
    ) -> Result<(bool, String)> {
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await
            && entries.next_entry().await.ok().flatten().is_some()
        {
            let message = "the folder is not empty".into();
            return Err(ToolFault::File { message }.into());
        }
        let side = self.dial_tool(session).await?;
        let conn = side.conn.as_ref();
        job.watch(conn.canceller());
        progress(Progress {
            done: 0,
            total: None,
            unit: Unit::Tables,
            rows: 0,
            bytes: 0,
            stage: Some("EXPORT DATABASE".into()),
        });
        let sql = format!("EXPORT DATABASE {}", sql_text(&dir.to_string_lossy()));
        let done = tokio::select! {
            r = collect(conn, &sql, None) => r,
            () = job.cancelled() => return Ok((true, String::new())),
        };
        done.map_err(|e| ToolFault::Failed {
            tool: "EXPORT DATABASE".into(),
            message: e.to_string(),
        })?;
        Ok((false, String::new()))
    }

    async fn duckdb_import(
        &self,
        session: &Session,
        job: &Job,
        dir: &Path,
    ) -> Result<(bool, String)> {
        if !tokio::fs::metadata(dir).await.is_ok_and(|m| m.is_dir()) {
            let message = "not an exported DuckDB folder".into();
            return Err(ToolFault::File { message }.into());
        }
        let side = self.dial_tool(session).await?;
        let conn = side.conn.as_ref();
        job.watch(conn.canceller());
        let sql = format!("IMPORT DATABASE {}", sql_text(&dir.to_string_lossy()));
        let done = tokio::select! {
            r = collect(conn, &sql, None) => r,
            () = job.cancelled() => return Ok((true, String::new())),
        };
        done.map_err(|e| ToolFault::Failed {
            tool: "IMPORT DATABASE".into(),
            message: e.to_string(),
        })?;
        Ok((false, String::new()))
    }

    async fn mssql_backup(&self, session: &Session, job: &Job, path: &str) -> Result<u64> {
        let side = self.dial_tool(session).await?;
        let conn = side.conn.as_ref();
        job.watch(conn.canceller());
        let name = collect(conn, "SELECT DB_NAME()", None).await;
        let name = name
            .ok()
            .and_then(|(r, _)| r.first()?.first().cloned().flatten())
            .ok_or(ToolFault::Empty)?;
        let file = path.replace('\'', "''");
        let sql = format!(
            "BACKUP DATABASE {} TO DISK = N'{file}' WITH INIT, COPY_ONLY, CHECKSUM",
            nd_sql::ident(Dialect::Mssql, &name)
        );
        let done = tokio::select! {
            r = collect(conn, &sql, None) => r,
            () = job.cancelled() => return Ok(0),
        };
        done.map_err(|e| ToolFault::Failed {
            tool: "BACKUP DATABASE".into(),
            message: e.to_string(),
        })?;
        let query = "SELECT TOP 1 CAST(b.backup_size AS varchar(30)) FROM msdb.dbo.backupset b \
JOIN msdb.dbo.backupmediafamily m ON m.media_set_id = b.media_set_id \
WHERE m.physical_device_name = @P1 ORDER BY b.backup_finish_date DESC";
        let found = collect(conn, query, Some(&[Some(path.to_owned())])).await;
        Ok(found
            .ok()
            .and_then(|(r, _)| r.first()?.first().cloned().flatten()?.parse().ok())
            .unwrap_or(0))
    }

    async fn server_backup(&self, session: &Session, job: &Job, path: &str) -> Result<u64> {
        if session.conn.dialect() == Some(Dialect::Clickhouse) {
            self.clickhouse_backup(session, job, path).await
        } else {
            self.mssql_backup(session, job, path).await
        }
    }

    async fn clickhouse_backup(&self, session: &Session, job: &Job, path: &str) -> Result<u64> {
        let side = self.dial_tool(session).await?;
        let conn = side.conn.as_ref();
        job.watch(conn.canceller());
        let database = session.conn.server().database.clone();
        let d = Dialect::Clickhouse;
        let sql = format!(
            "BACKUP DATABASE {} TO File({})",
            nd_sql::ident(d, &database),
            nd_sql::literal(d, Some(path))
        );
        let done = tokio::select! {
            r = collect(conn, &sql, None) => r,
            () = job.cancelled() => return Ok(0),
        };
        let (rows, _) = done.map_err(|e| ToolFault::Failed {
            tool: "BACKUP DATABASE".into(),
            message: e.to_string(),
        })?;
        let Some(id) = rows.first().and_then(|r| r.first().cloned().flatten()) else {
            return Ok(0);
        };
        let query = format!(
            "SELECT compressed_size FROM system.backups WHERE id = {}",
            nd_sql::literal(d, Some(&id))
        );
        let found = collect(conn, &query, None).await;
        Ok(found
            .ok()
            .and_then(|(r, _)| r.first()?.first().cloned().flatten()?.parse().ok())
            .unwrap_or(0))
    }

    async fn key_copy(&self, profile: &nd_store::ConnectionProfile) -> Result<Option<KeyFile>> {
        let Some(cert) = profile.target.client_cert() else {
            return Ok(None);
        };
        if !nd_driver::key_encrypted(cert)? {
            return Ok(None);
        }
        let pass = Zeroizing::new(
            self.secret(&profile.id, "tls-key")
                .await?
                .unwrap_or_default(),
        );
        Ok(nd_driver::key_file(cert, Some(&pass))?)
    }

    async fn password_of(&self, id: &str) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(
            self.secret(id, "password").await?.unwrap_or_default(),
        ))
    }

    #[allow(clippy::too_many_lines)]
    pub async fn restore(
        &self,
        req: RestoreRequest,
        progress: impl Fn(Progress) + Send + Sync,
    ) -> Result<Backed> {
        let session = self.live(&req.session_id).await?;
        let confirm = req.confirm.as_deref();
        self.authorize(&session, Access::Write { confirm })?;
        let profile = self.profile_of(&session)?;
        if matches!(
            profile.target,
            Target::Sqlserver(_)
                | Target::Cockroach(_)
                | Target::Libsql(LibsqlTarget::Remote { .. })
        ) {
            return Err(ToolFault::Unsupported.into());
        }
        let path = match &profile.target {
            Target::Duckdb(_) => PathBuf::from(&req.path),
            _ => checked(&req.path)?,
        };
        let slot = self.job_start(&req.job_id)?;
        let started = Instant::now();
        let password = self.password_of(&profile.id).await?;
        let key = self.key_copy(&profile).await?;
        let magic = if matches!(profile.target, Target::Duckdb(_)) {
            Vec::new()
        } else {
            head(&path, 16).await?
        };
        let (pg, my) = self.tool_dirs()?;
        let run = |tool: String| Run {
            job: &slot.job,
            progress: &progress,
            tool,
        };
        let outcome = match &profile.target {
            Target::Sqlite(SqliteTarget {
                path: to,
                read_only,
            })
            | Target::Libsql(LibsqlTarget::File {
                path: to,
                read_only,
            }) => {
                let job = slot.job.clone();
                sqlite_restore(&path, to, *read_only, &magic, job, &progress).await
            }
            Target::Duckdb(t) => {
                if t.read_only {
                    return Err(CoreError::ReadOnly);
                }
                self.duckdb_import(&session, &slot.job, &path).await
            }
            Target::Sqlserver(_)
            | Target::Cockroach(_)
            | Target::Libsql(LibsqlTarget::Remote { .. })
            | Target::Mongodb(_)
            | Target::Clickhouse(_)
            | Target::Redis(_)
            | Target::Cassandra(_)
            | Target::Dynamodb(_)
            | Target::Elasticsearch(_)
            | Target::Firebird(_)
            | Target::Oracle(_)
            | Target::Db2(_)
            | Target::Hana(_)
            | Target::Odbc(_) => {
                return Err(ToolFault::Unsupported.into());
            }
            Target::Postgres(t) => {
                let archive = magic.starts_with(PG_MAGIC);
                let name = if archive { "pg_restore" } else { "psql" };
                let reach = Reach::of_pg(&session, t, &password).key(key.as_ref());
                let (mut cmd, _pass) = pg_command(pg_program(name, pg.as_deref())?, &reach)?;
                if archive {
                    cmd.args(["--verbose", "--single-transaction", "--exit-on-error"]);
                } else {
                    cmd.args(["-X", "-q", "-1", "-v", "ON_ERROR_STOP=1"]);
                }
                run(name.into()).go(cmd, None, Some(&path), None).await
            }
            Target::Mysql(t) => {
                let (_, names) = mysql_names(mariadb(&session));
                let found = mysql_program(names, my.as_deref()).await?;
                let (mut cmd, _pass) = mysql_command(
                    &found,
                    &Reach::of_mysql(&session, t, &password).key(key.as_ref()),
                )?;
                cmd.arg(&t.database);
                run(found.name).go(cmd, None, Some(&path), None).await
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let error = outcome.as_ref().err().map(ToString::to_string);
        self.audit(&session, &format!("-- restore {file}"), None, error);
        let (cancelled, notes) = outcome?;
        Ok(Backed {
            bytes: tree_size(&path).await,
            elapsed_ms: ms(started),
            cancelled,
            notes: Some(tail(&notes, 6)).filter(|n| !n.is_empty()),
        })
    }
}

async fn sqlite_restore(
    from: &Path,
    to: &str,
    read_only: bool,
    magic: &[u8],
    job: Arc<Job>,
    progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<(bool, String)> {
    if read_only {
        return Err(CoreError::ReadOnly);
    }
    if !magic.starts_with(SQLITE_MAGIC) {
        let message = "not a SQLite database".into();
        return Err(ToolFault::File { message }.into());
    }
    sqlite_job(from.to_path_buf(), PathBuf::from(to), job, progress)
        .await
        .map(|c| (c, String::new()))
}

fn mariadb(session: &Session) -> bool {
    session.conn.server().product.contains("MariaDB")
}

struct Reach<'a> {
    host: String,
    port: String,
    user: &'a str,
    database: &'a str,
    tls: TlsMode,
    ca: Option<&'a str>,
    cert: Option<&'a nd_driver::ClientCert>,
    key: Option<&'a Path>,
    password: Option<&'a str>,
}

impl<'a> Reach<'a> {
    fn of_pg(session: &Session, t: &'a nd_driver::PostgresTarget, password: &'a str) -> Self {
        let (host, port) = endpoint(session, &t.host, t.port);
        Self {
            host,
            port: port.to_string(),
            user: &t.user,
            database: &t.database,
            tls: t.tls,
            ca: t.ca_file.as_deref(),
            cert: t.client_cert.as_ref().filter(|_| t.tls != TlsMode::Disable),
            key: None,
            password: Some(password).filter(|p| !p.is_empty()),
        }
    }

    fn of_mysql(session: &Session, t: &'a nd_driver::MysqlTarget, password: &'a str) -> Self {
        let (host, port) = endpoint(session, &t.host, t.port);
        Self {
            host,
            port: port.to_string(),
            user: &t.user,
            database: &t.database,
            tls: t.tls,
            ca: t.ca_file.as_deref(),
            cert: t.client_cert.as_ref().filter(|_| t.tls != TlsMode::Disable),
            key: None,
            password: Some(password).filter(|p| !p.is_empty()),
        }
    }

    fn key(mut self, copy: Option<&'a KeyFile>) -> Self {
        self.key = copy.map(KeyFile::path);
        self
    }

    fn key_path(&self) -> Option<String> {
        self.key
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| self.cert.map(|c| c.key.clone()))
    }
}

fn pg_program(name: &str, chosen: Option<&Path>) -> Result<PathBuf> {
    find(name, ToolFamily::Postgres, chosen)
        .ok_or_else(|| ToolFault::Missing { tool: name.into() }.into())
}

async fn mysql_program(names: [&str; 2], chosen: Option<&Path>) -> Result<BackupTool> {
    first_of(names, ToolFamily::Mysql, chosen)
        .await
        .ok_or_else(|| {
            ToolFault::Missing {
                tool: names[0].into(),
            }
            .into()
        })
}

fn pg_command(program: PathBuf, r: &Reach<'_>) -> Result<(Command, Option<PrivateDir>)> {
    let mut cmd = Command::new(program);
    cmd.args([
        "--no-password",
        "-h",
        &r.host,
        "-p",
        &r.port,
        "-U",
        r.user,
        "-d",
        r.database,
    ]);
    pg_env(&mut cmd, r.tls, r.ca);
    if let (Some(c), Some(key)) = (r.cert, r.key_path()) {
        cmd.env("PGSSLCERT", &c.cert).env("PGSSLKEY", key);
    }
    let mut kept = None;
    match r.password.map(|p| (p, pgpass(p))) {
        Some((_, Some(line))) => {
            let (dir, path) = private_file("pgpass", &line)?;
            cmd.env("PGPASSFILE", path);
            kept = Some(dir);
        }
        Some((p, None)) => {
            cmd.env("PGPASSWORD", p);
        }
        None => {}
    }
    Ok((cmd, kept))
}

fn mysql_command(found: &BackupTool, r: &Reach<'_>) -> Result<(Command, Option<PrivateDir>)> {
    let maria_client = found
        .version
        .as_deref()
        .is_some_and(|v| v.contains("MariaDB"));
    let mut cmd = Command::new(found.path.clone().unwrap_or_default());
    let mut kept = None;
    if let Some(p) = r.password {
        let (dir, path) = private_file("my.cnf", &mysql_options(p))?;
        let mut arg = std::ffi::OsString::from("--defaults-extra-file=");
        arg.push(path);
        cmd.arg(arg);
        kept = Some(dir);
    }
    cmd.args(["--protocol=TCP", "-h", &r.host, "-P", &r.port, "-u", r.user]);
    mysql_tls(&mut cmd, r.tls, r.ca, maria_client);
    if let (Some(c), Some(key)) = (r.cert, r.key_path()) {
        cmd.arg(format!("--ssl-cert={}", c.cert))
            .arg(format!("--ssl-key={key}"));
    }
    Ok((cmd, kept))
}
