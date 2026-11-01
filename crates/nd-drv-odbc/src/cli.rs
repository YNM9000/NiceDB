#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};

use libloading::Library;
use nd_driver::{CatalogRows, Column, DriverError, Result};

use crate::client::{Backend, Stop};

type Handle = i32;
type Ret = i16;

const SQL_SUCCESS: Ret = 0;
const SQL_SUCCESS_WITH_INFO: Ret = 1;
const SQL_NO_DATA: Ret = 100;
const HANDLE_ENV: i16 = 1;
const HANDLE_DBC: i16 = 2;
const HANDLE_STMT: i16 = 3;
const ATTR_ODBC_VERSION: i32 = 200;
const OV_ODBC3: usize = 3;
const ATTR_AUTOCOMMIT: i32 = 102;
const DRIVER_NOPROMPT: u16 = 0;
const NULL_DATA: i32 = -1;
const NO_TOTAL: i32 = -4;
const C_WCHAR: i16 = -8;
const C_BINARY: i16 = -2;
const PARAM_INPUT: i16 = 1;
const SQL_WVARCHAR: i16 = -9;
const CLOSE: u16 = 0;
const COMMIT: i16 = 0;
const ROLLBACK: i16 = 1;
const BINARY_TYPES: [i16; 4] = [-2, -3, -4, -98];
const CHUNK: usize = 8192;

type AllocHandle = unsafe extern "C" fn(i16, Handle, *mut Handle) -> Ret;
type FreeHandle = unsafe extern "C" fn(i16, Handle) -> Ret;
type SetAttr = unsafe extern "C" fn(Handle, i32, *mut c_void, i32) -> Ret;
type DriverConnect =
    unsafe extern "C" fn(Handle, *mut c_void, *const u16, i16, *mut u16, i16, *mut i16, u16) -> Ret;
type OneHandle = unsafe extern "C" fn(Handle) -> Ret;
type Prepare = unsafe extern "C" fn(Handle, *const u16, i32) -> Ret;
type BindParameter =
    unsafe extern "C" fn(Handle, u16, i16, i16, i16, u32, i16, *mut c_void, i32, *mut i32) -> Ret;
type NumCols = unsafe extern "C" fn(Handle, *mut i16) -> Ret;
type DescribeCol = unsafe extern "C" fn(
    Handle,
    u16,
    *mut u16,
    i16,
    *mut i16,
    *mut i16,
    *mut u32,
    *mut i16,
    *mut i16,
) -> Ret;
type GetData = unsafe extern "C" fn(Handle, u16, i16, *mut c_void, i32, *mut i32) -> Ret;
type RowCount = unsafe extern "C" fn(Handle, *mut i32) -> Ret;
type FreeStmt = unsafe extern "C" fn(Handle, u16) -> Ret;
type EndTran = unsafe extern "C" fn(i16, Handle, i16) -> Ret;
type DiagRec =
    unsafe extern "C" fn(i16, Handle, i16, *mut u16, *mut i32, *mut u16, i16, *mut i16) -> Ret;

struct Api {
    alloc: AllocHandle,
    free: FreeHandle,
    set_env: SetAttr,
    set_conn: SetAttr,
    connect: DriverConnect,
    disconnect: OneHandle,
    prepare: Prepare,
    bind: BindParameter,
    execute: OneHandle,
    num_cols: NumCols,
    describe: DescribeCol,
    fetch: OneHandle,
    get_data: GetData,
    row_count: RowCount,
    free_stmt: FreeStmt,
    cancel: OneHandle,
    end_tran: EndTran,
    diag: DiagRec,
    _lib: Library,
}

fn load_error(e: impl std::fmt::Display) -> DriverError {
    DriverError::Connect {
        message: format!("IBM Db2 CLI driver: {e}"),
    }
}

impl Api {
    fn load(path: &Path) -> Result<Self> {
        let lib = unsafe { Library::new(path) }.map_err(load_error)?;
        macro_rules! sym {
            ($name:literal) => {
                *unsafe { lib.get(concat!($name, "\0").as_bytes()) }.map_err(load_error)?
            };
        }
        Ok(Self {
            alloc: sym!("SQLAllocHandle"),
            free: sym!("SQLFreeHandle"),
            set_env: sym!("SQLSetEnvAttr"),
            set_conn: sym!("SQLSetConnectAttr"),
            connect: sym!("SQLDriverConnectW"),
            disconnect: sym!("SQLDisconnect"),
            prepare: sym!("SQLPrepareW"),
            bind: sym!("SQLBindParameter"),
            execute: sym!("SQLExecute"),
            num_cols: sym!("SQLNumResultCols"),
            describe: sym!("SQLDescribeColW"),
            fetch: sym!("SQLFetch"),
            get_data: sym!("SQLGetData"),
            row_count: sym!("SQLRowCount"),
            free_stmt: sym!("SQLFreeStmt"),
            cancel: sym!("SQLCancel"),
            end_tran: sym!("SQLEndTran"),
            diag: sym!("SQLGetDiagRecW"),
            _lib: lib,
        })
    }

    fn error(&self, kind: i16, handle: Handle, fallback: &str) -> DriverError {
        let mut state = [0u16; 6];
        let mut native = 0i32;
        let mut text = vec![0u16; 1024];
        let mut len = 0i16;
        let ret = unsafe {
            (self.diag)(
                kind,
                handle,
                1,
                state.as_mut_ptr(),
                &raw mut native,
                text.as_mut_ptr(),
                1024,
                &raw mut len,
            )
        };
        if !ok(ret) {
            return DriverError::query(fallback);
        }
        let used = usize::try_from(len).unwrap_or(0).min(text.len());
        DriverError::Query {
            message: String::from_utf16_lossy(&text[..used]).trim().to_owned(),
            code: Some(String::from_utf16_lossy(&state[..5])),
            detail: None,
            hint: None,
            offset: None,
        }
    }

    fn check(&self, ret: Ret, kind: i16, handle: Handle, what: &str) -> Result<()> {
        if ok(ret) {
            Ok(())
        } else {
            Err(self.error(kind, handle, what))
        }
    }
}

fn loaded(path: &Path) -> Result<Arc<Api>> {
    static LOADED: Mutex<Vec<(std::path::PathBuf, Arc<Api>)>> = Mutex::new(Vec::new());
    let mut all = LOADED
        .lock()
        .map_err(|_| load_error("library table poisoned"))?;
    if let Some((_, api)) = all.iter().find(|(p, _)| p == path) {
        return Ok(api.clone());
    }
    let api = Arc::new(Api::load(path)?);
    all.push((path.to_path_buf(), api.clone()));
    Ok(api)
}

fn ok(ret: Ret) -> bool {
    ret == SQL_SUCCESS || ret == SQL_SUCCESS_WITH_INFO
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn wide_len(v: &[u16]) -> Result<i32> {
    i32::try_from(v.len()).map_err(|_| DriverError::query("statement too long"))
}

#[derive(Clone)]
pub(crate) struct CliStop {
    api: Arc<Api>,
    running: Arc<Mutex<Option<Handle>>>,
}

impl Stop for CliStop {
    fn stop(&self) -> bool {
        let Ok(h) = self.running.lock() else {
            return false;
        };
        let Some(stmt) = *h else { return false };
        ok(unsafe { (self.api.cancel)(stmt) })
    }
}

pub(crate) struct CliBackend {
    api: Arc<Api>,
    env: Handle,
    dbc: Handle,
    running: Arc<Mutex<Option<Handle>>>,
}

impl CliBackend {
    pub(crate) fn open(library: &Path, text: &str) -> Result<(Self, CliStop)> {
        let api = loaded(library)?;
        let mut env: Handle = 0;
        let ret = unsafe { (api.alloc)(HANDLE_ENV, 0, &raw mut env) };
        if !ok(ret) {
            return Err(load_error("no environment handle"));
        }
        unsafe { (api.set_env)(env, ATTR_ODBC_VERSION, OV_ODBC3 as *mut c_void, 0) };
        let mut dbc: Handle = 0;
        let ret = unsafe { (api.alloc)(HANDLE_DBC, env, &raw mut dbc) };
        if !ok(ret) {
            let e = api.error(HANDLE_ENV, env, "no connection handle");
            unsafe { (api.free)(HANDLE_ENV, env) };
            return Err(e);
        }
        let input = wide(text);
        let mut out = vec![0u16; 1024];
        let mut out_len = 0i16;
        let len =
            i16::try_from(input.len()).map_err(|_| load_error("connection string too long"))?;
        let ret = unsafe {
            (api.connect)(
                dbc,
                std::ptr::null_mut(),
                input.as_ptr(),
                len,
                out.as_mut_ptr(),
                1024,
                &raw mut out_len,
                DRIVER_NOPROMPT,
            )
        };
        if !ok(ret) {
            let e = api.error(HANDLE_DBC, dbc, "connection refused");
            unsafe {
                (api.free)(HANDLE_DBC, dbc);
                (api.free)(HANDLE_ENV, env);
            }
            return Err(DriverError::Connect {
                message: e.to_string(),
            });
        }
        let running = Arc::new(Mutex::new(None));
        let stop = CliStop {
            api: api.clone(),
            running: running.clone(),
        };
        Ok((
            Self {
                api,
                env,
                dbc,
                running,
            },
            stop,
        ))
    }

    fn mark(&self, stmt: Option<Handle>) {
        if let Ok(mut h) = self.running.lock() {
            *h = stmt;
        }
    }

    fn cell(&self, stmt: Handle, col: u16, binary: bool) -> Result<Option<String>> {
        let mut bytes: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; CHUNK];
        let cap = i32::try_from(CHUNK).unwrap_or(i32::MAX);
        let terminator = if binary { 0 } else { 2 };
        loop {
            let mut ind = 0i32;
            let ret = unsafe {
                (self.api.get_data)(
                    stmt,
                    col,
                    if binary { C_BINARY } else { C_WCHAR },
                    buf.as_mut_ptr().cast(),
                    cap,
                    &raw mut ind,
                )
            };
            if ret == SQL_NO_DATA {
                break;
            }
            self.api
                .check(ret, HANDLE_STMT, stmt, "cannot read a value")?;
            if ind == NULL_DATA {
                return Ok(None);
            }
            let room = CHUNK - terminator;
            let got = match ind {
                NO_TOTAL => room,
                n if n >= 0 => usize::try_from(n).unwrap_or(room).min(room),
                n => {
                    return Err(DriverError::query(format!(
                        "bad length {n} from the driver"
                    )));
                }
            };
            bytes.extend_from_slice(&buf[..got]);
            let truncated = ret == SQL_SUCCESS_WITH_INFO
                && (ind == NO_TOTAL || usize::try_from(ind).is_ok_and(|n| n > room));
            if !truncated {
                break;
            }
        }
        if binary {
            let mut s = String::with_capacity(2 + bytes.len() * 2);
            s.push_str("\\x");
            for b in &bytes {
                let _ = write!(s, "{b:02x}");
            }
            return Ok(Some(s));
        }
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_ne_bytes(*p))
            .collect();
        Ok(Some(String::from_utf16_lossy(&units)))
    }

    fn columns(&self, stmt: Handle) -> Result<(Vec<Column>, Vec<bool>)> {
        let mut n = 0i16;
        let ret = unsafe { (self.api.num_cols)(stmt, &raw mut n) };
        self.api
            .check(ret, HANDLE_STMT, stmt, "cannot count columns")?;
        let mut cols = Vec::new();
        let mut binary = Vec::new();
        for i in 1..=u16::try_from(n.max(0)).unwrap_or(0) {
            let mut name = vec![0u16; 256];
            let (mut len, mut ty, mut size, mut digits, mut nullable) =
                (0i16, 0i16, 0u32, 0i16, 0i16);
            let ret = unsafe {
                (self.api.describe)(
                    stmt,
                    i,
                    name.as_mut_ptr(),
                    256,
                    &raw mut len,
                    &raw mut ty,
                    &raw mut size,
                    &raw mut digits,
                    &raw mut nullable,
                )
            };
            self.api
                .check(ret, HANDLE_STMT, stmt, "cannot describe a column")?;
            let used = usize::try_from(len).unwrap_or(0).min(name.len());
            cols.push(Column {
                name: String::from_utf16_lossy(&name[..used]),
                type_name: Some(type_name(ty, size, digits)),
            });
            binary.push(BINARY_TYPES.contains(&ty));
        }
        Ok((cols, binary))
    }

    fn statement(
        &self,
        stmt: Handle,
        sql: &str,
        params: &[Option<String>],
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<bool>,
    ) -> Result<Option<u64>> {
        let text = wide(sql);
        let ret = unsafe { (self.api.prepare)(stmt, text.as_ptr(), wide_len(&text)?) };
        self.api.check(ret, HANDLE_STMT, stmt, "cannot prepare")?;
        let mut values: Vec<Vec<u16>> = params
            .iter()
            .map(|p| p.as_deref().map(wide).unwrap_or_default())
            .collect();
        let mut lens: Vec<i32> = Vec::with_capacity(params.len());
        for (v, p) in values.iter().zip(params) {
            lens.push(if p.is_some() {
                i32::try_from(v.len() * 2).map_err(|_| DriverError::query("value too long"))?
            } else {
                NULL_DATA
            });
        }
        for (i, v) in values.iter_mut().enumerate() {
            let n = u16::try_from(i + 1).map_err(|_| DriverError::query("too many parameters"))?;
            let size = u32::try_from(v.len().max(1)).unwrap_or(u32::MAX);
            let bytes = i32::try_from(v.len() * 2).unwrap_or(i32::MAX);
            let ret = unsafe {
                (self.api.bind)(
                    stmt,
                    n,
                    PARAM_INPUT,
                    C_WCHAR,
                    SQL_WVARCHAR,
                    size,
                    0,
                    v.as_mut_ptr().cast(),
                    bytes,
                    &raw mut lens[i],
                )
            };
            self.api
                .check(ret, HANDLE_STMT, stmt, "cannot bind a parameter")?;
        }
        let ret = unsafe { (self.api.execute)(stmt) };
        if ret != SQL_NO_DATA {
            self.api.check(ret, HANDLE_STMT, stmt, "cannot execute")?;
        }
        let (cols, binary) = self.columns(stmt)?;
        if cols.is_empty() {
            let mut n = 0i32;
            let ret = unsafe { (self.api.row_count)(stmt, &raw mut n) };
            return Ok(if ok(ret) { u64::try_from(n).ok() } else { None });
        }
        head(cols)?;
        loop {
            let ret = unsafe { (self.api.fetch)(stmt) };
            if ret == SQL_NO_DATA {
                break;
            }
            self.api.check(ret, HANDLE_STMT, stmt, "cannot fetch")?;
            let mut cells = Vec::with_capacity(binary.len());
            for (i, bin) in binary.iter().enumerate() {
                let col = u16::try_from(i + 1).unwrap_or(u16::MAX);
                cells.push(self.cell(stmt, col, *bin)?);
            }
            if !row(cells)? {
                break;
            }
        }
        Ok(None)
    }
}

fn type_name(ty: i16, size: u32, digits: i16) -> String {
    match ty {
        1 => format!("CHAR({size})"),
        12 | -9 => format!("VARCHAR({size})"),
        3 | 2 => format!("DECIMAL({size},{digits})"),
        4 => "INTEGER".into(),
        5 => "SMALLINT".into(),
        -5 => "BIGINT".into(),
        7 => "REAL".into(),
        6 | 8 => "DOUBLE".into(),
        91 => "DATE".into(),
        92 => "TIME".into(),
        93 => "TIMESTAMP".into(),
        16 => "BOOLEAN".into(),
        -99 | -1 | -10 => "CLOB".into(),
        -350 => "DBCLOB".into(),
        -98 | -4 => "BLOB".into(),
        -2 => format!("BINARY({size})"),
        -3 => format!("VARBINARY({size})"),
        -360 => "DECFLOAT".into(),
        -370 => "XML".into(),
        other => format!("TYPE {other}"),
    }
}

impl Backend for CliBackend {
    fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<bool>,
    ) -> Result<Option<u64>> {
        let mut stmt: Handle = 0;
        let ret = unsafe { (self.api.alloc)(HANDLE_STMT, self.dbc, &raw mut stmt) };
        self.api
            .check(ret, HANDLE_DBC, self.dbc, "no statement handle")?;
        self.mark(Some(stmt));
        let out = self.statement(stmt, sql, params, head, row);
        self.mark(None);
        unsafe {
            (self.api.free_stmt)(stmt, CLOSE);
            (self.api.free)(HANDLE_STMT, stmt);
        }
        out
    }

    fn autocommit(&mut self, on: bool) -> Result<()> {
        let ret = unsafe {
            (self.api.set_conn)(self.dbc, ATTR_AUTOCOMMIT, usize::from(on) as *mut c_void, 0)
        };
        self.api
            .check(ret, HANDLE_DBC, self.dbc, "cannot set autocommit")
    }

    fn end(&mut self, commit: bool) -> Result<()> {
        let how = if commit { COMMIT } else { ROLLBACK };
        let ret = unsafe { (self.api.end_tran)(HANDLE_DBC, self.dbc, how) };
        self.api
            .check(ret, HANDLE_DBC, self.dbc, "cannot end the transaction")
    }

    fn catalog(&mut self) -> Result<CatalogRows> {
        Err(DriverError::query("the Db2 catalog is read from SYSCAT"))
    }

    fn product(&mut self) -> String {
        "Db2".into()
    }

    fn database(&mut self) -> String {
        let mut name = String::new();
        let _ = self.run(
            "SELECT RTRIM(CURRENT SERVER) FROM SYSIBM.SYSDUMMY1",
            &[],
            &mut |_| Ok(()),
            &mut |r| {
                name = r.into_iter().next().flatten().unwrap_or_default();
                Ok(false)
            },
        );
        name
    }
}

impl Drop for CliBackend {
    fn drop(&mut self) {
        unsafe {
            (self.api.disconnect)(self.dbc);
            (self.api.free)(HANDLE_DBC, self.dbc);
            (self.api.free)(HANDLE_ENV, self.env);
        }
    }
}
