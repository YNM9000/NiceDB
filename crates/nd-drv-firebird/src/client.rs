use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nd_driver::{Column, DriverError, Result};
use rsfbclient_core::{
    Dialect, FbError, FirebirdClientDbOps, FirebirdClientSqlOps, FreeStmtOp, SqlType, StmtType,
    TrDataAccessMode, TrOp, TransactionConfiguration,
};
use rsfbclient_rust::{
    ColumnInfo, DbHandle, RustFbClient, RustFbClientAttachmentConfig, StmtHandleData, TrHandle,
};

use crate::value;

pub(crate) struct Fb {
    cli: RustFbClient,
    db: DbHandle,
    open: Option<TrHandle>,
    pub(crate) read_only: Arc<AtomicBool>,
}

pub(crate) type Rows = Vec<Vec<Option<String>>>;

pub(crate) enum Ran {
    Count(u64),
    Done,
}

pub(crate) fn map_err(e: FbError) -> DriverError {
    match e {
        FbError::Sql { msg, code } => DriverError::Query {
            message: msg.trim_end().to_owned(),
            code: Some(code.to_string()),
            detail: None,
            hint: None,
            offset: None,
        },
        FbError::Io(e) => DriverError::Io {
            message: e.to_string(),
        },
        FbError::Other(m) => DriverError::query(m),
    }
}

impl Fb {
    pub(crate) fn attach(config: &RustFbClientAttachmentConfig) -> Result<Self> {
        let mut cli = RustFbClient::new(rsfbclient_core::charset::UTF_8);
        let db = cli
            .attach_database(config, Dialect::D3, false)
            .map_err(|e| DriverError::Connect {
                message: e.to_string(),
            })?;
        Ok(Self {
            cli,
            db,
            open: None,
            read_only: Arc::default(),
        })
    }

    fn config(&self) -> TransactionConfiguration {
        TransactionConfiguration {
            data_access: if self.read_only.load(Ordering::Acquire) {
                TrDataAccessMode::ReadOnly
            } else {
                TrDataAccessMode::ReadWrite
            },
            ..TransactionConfiguration::default()
        }
    }

    pub(crate) fn begin(&mut self) -> Result<()> {
        if self.open.is_none() {
            let config = self.config();
            let tr = self
                .cli
                .begin_transaction(&mut self.db, config)
                .map_err(map_err)?;
            self.open = Some(tr);
        }
        Ok(())
    }

    pub(crate) fn end(&mut self, commit: bool) -> Result<()> {
        if let Some(mut tr) = self.open.take() {
            let op = if commit { TrOp::Commit } else { TrOp::Rollback };
            self.cli
                .transaction_operation(&mut tr, op)
                .map_err(map_err)?;
        }
        Ok(())
    }

    fn within<T>(
        &mut self,
        work: impl FnOnce(&mut RustFbClient, &mut DbHandle, &mut TrHandle) -> Result<T>,
    ) -> Result<T> {
        if let Some(tr) = self.open.as_mut() {
            return work(&mut self.cli, &mut self.db, tr);
        }
        let config = self.config();
        let mut tr = self
            .cli
            .begin_transaction(&mut self.db, config)
            .map_err(map_err)?;
        let out = work(&mut self.cli, &mut self.db, &mut tr);
        let op = if out.is_ok() {
            TrOp::Commit
        } else {
            TrOp::Rollback
        };
        let ended = self.cli.transaction_operation(&mut tr, op).map_err(map_err);
        let out = out?;
        ended?;
        Ok(out)
    }

    pub(crate) fn run(
        &mut self,
        sql: &str,
        params: Vec<SqlType>,
        head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
        row: &mut dyn FnMut(Vec<Option<String>>) -> Result<()>,
    ) -> Result<Ran> {
        self.within(|cli, db, tr| {
            let (kind, mut stmt) = cli
                .prepare_statement(db, tr, Dialect::D3, sql)
                .map_err(map_err)?;
            let out = drive(cli, db, tr, kind, &mut stmt, params, head, row);
            let _ = cli.free_statement(&mut stmt, FreeStmtOp::Drop);
            out
        })
    }

    pub(crate) fn query(&mut self, sql: &str, params: Vec<SqlType>) -> Result<Rows> {
        let mut rows = Vec::new();
        self.run(sql, params, &mut |_| Ok(()), &mut |r| {
            rows.push(r);
            Ok(())
        })?;
        Ok(rows)
    }

    pub(crate) fn plan(&mut self, sql: &str) -> Result<String> {
        let mut tr = self
            .cli
            .begin_transaction(&mut self.db, TransactionConfiguration::default())
            .map_err(map_err)?;
        let planned = self
            .cli
            .prepare_statement(&mut self.db, &mut tr, Dialect::D3, sql)
            .map_err(map_err)
            .and_then(|(_, mut stmt)| {
                let plan = self.cli.plan(&stmt).map_err(map_err);
                let _ = self.cli.free_statement(&mut stmt, FreeStmtOp::Drop);
                plan
            });
        let _ = self.cli.transaction_operation(&mut tr, TrOp::Rollback);
        planned
    }

    pub(crate) fn detach(mut self) {
        let _ = self.end(false);
        let _ = self.cli.detach_database(&mut self.db);
    }
}

fn header(columns: &[ColumnInfo]) -> Vec<Column> {
    columns
        .iter()
        .map(|c| Column {
            name: c.name.to_string(),
            type_name: Some(value::type_name(c)),
        })
        .collect()
}

fn cells(row: &[rsfbclient_core::Column]) -> Vec<Option<String>> {
    row.iter().map(|c| value::display(&c.value)).collect()
}

#[allow(clippy::too_many_arguments)]
fn drive(
    cli: &mut RustFbClient,
    db: &mut DbHandle,
    tr: &mut TrHandle,
    kind: StmtType,
    stmt: &mut StmtHandleData,
    params: Vec<SqlType>,
    head: &mut dyn FnMut(Vec<Column>) -> Result<()>,
    row: &mut dyn FnMut(Vec<Option<String>>) -> Result<()>,
) -> Result<Ran> {
    let columns = stmt.columns();
    if matches!(kind, StmtType::Select | StmtType::SelectForUpd) {
        head(header(&columns))?;
        cli.execute(db, tr, stmt, params).map_err(map_err)?;
        while let Some(r) = cli.fetch(db, tr, stmt).map_err(map_err)? {
            row(cells(&r))?;
        }
        return Ok(Ran::Done);
    }
    if columns.is_empty() {
        let n = cli.execute(db, tr, stmt, params).map_err(map_err)?;
        return Ok(match kind {
            StmtType::Insert | StmtType::Update | StmtType::Delete => Ran::Count(n as u64),
            _ => Ran::Done,
        });
    }
    head(header(&columns))?;
    let r = cli.execute2(db, tr, stmt, params).map_err(map_err)?;
    if !r.is_empty() {
        row(cells(&r))?;
    }
    Ok(Ran::Done)
}
