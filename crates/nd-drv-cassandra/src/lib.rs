mod cql;
mod schema;
mod value;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nd_driver::{
    Canceller, Capabilities, CassandraTarget, Chunk, CommandClass, Commands, Consistency, DdlKind,
    Dependency, DocumentWrite, DriverError, Family, Group, Language, NamespaceLevel, Noun,
    ObjectRef, Outcome, Report, ReportGroup, ReportSection, Result, ResultShape, Schema,
    SchemaKind, ServerInfo, Tool, TreeShape, Vocabulary,
};
use scylla::client::execution_profile::ExecutionProfile;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::errors::TranslationError;
use scylla::policies::address_translator::{AddressTranslator, UntranslatedPeer};
use scylla::policies::load_balancing::DefaultPolicy;
use scylla::response::{PagingState, PagingStateResponse};
use scylla::statement::Statement;
use scylla::value::{CqlValue, Row};
use serde_json::Value;
use tokio::sync::{Notify, mpsc};

use crate::cql::{json_row, literal, quote};

const PAGE: i32 = 1000;

pub fn capabilities() -> Capabilities {
    Capabilities {
        family: Family::WideColumn,
        language: Language::Cql,
        schema: SchemaKind::Declared,
        results: ResultShape::Documents,
        tree: TreeShape {
            databases: true,
            namespaces: NamespaceLevel::Multiple,
            groups: vec![Group::Tables, Group::MaterializedViews, Group::Types],
            server: false,
        },
        words: Vocabulary {
            table: Noun::Table,
            column: Noun::Column,
        },
        transactions: false,
        editable: true,
        explain: true,
        er: false,
        ddl: true,
        sort: false,
        params: false,
        tools: vec![Tool::Cluster],
        cancel: nd_driver::Cancel::BetweenFetches,
    }
}

fn level(c: Consistency) -> scylla::statement::Consistency {
    use scylla::statement::Consistency as C;
    match c {
        Consistency::Any => C::Any,
        Consistency::One => C::One,
        Consistency::Two => C::Two,
        Consistency::Three => C::Three,
        Consistency::Quorum => C::Quorum,
        Consistency::All => C::All,
        Consistency::LocalQuorum => C::LocalQuorum,
        Consistency::EachQuorum => C::EachQuorum,
        Consistency::LocalOne => C::LocalOne,
    }
}

fn query_err(e: impl std::fmt::Display) -> DriverError {
    DriverError::query(e.to_string())
}

struct ThroughTunnel(SocketAddr);

#[async_trait::async_trait]
impl AddressTranslator for ThroughTunnel {
    async fn translate_address(
        &self,
        _peer: &UntranslatedPeer,
    ) -> std::result::Result<SocketAddr, TranslationError> {
        Ok(self.0)
    }
}

struct Cancel {
    stop: Notify,
}

struct CassandraCanceller(Arc<Cancel>);

#[async_trait::async_trait]
impl Canceller for CassandraCanceller {
    async fn cancel(&self) {
        self.0.stop.notify_waiters();
    }
}

pub struct CassandraConnection {
    session: Session,
    cancel: Arc<Cancel>,
    server: ServerInfo,
    consistency: Mutex<Consistency>,
    keys: Mutex<HashMap<(String, String), Vec<String>>>,
}

fn tls_config(
    t: &CassandraTarget,
    key_passphrase: Option<&str>,
) -> Result<Option<Arc<rustls::ClientConfig>>> {
    if t.tls == nd_driver::TlsMode::Disable {
        return Ok(None);
    }
    let identity = match &t.client_cert {
        Some(c) => Some(nd_driver::load_identity(c, key_passphrase)?),
        None => None,
    };
    nd_http::tls_config(
        t.tls,
        t.ca_file.as_deref().filter(|f| !f.is_empty()),
        identity,
    )
}

pub async fn connect(
    t: &CassandraTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<CassandraConnection> {
    let mut builder = SessionBuilder::new().connection_timeout(Duration::from_secs(10));
    builder = match via {
        Some((ip, port)) => {
            let at = SocketAddr::new(ip, port);
            builder
                .known_node_addr(at)
                .address_translator(Arc::new(ThroughTunnel(at)))
        }
        None if t.hosts().count() == 1 => {
            let at = tokio::net::lookup_host((t.first_host(), t.port))
                .await
                .ok()
                .and_then(|mut a| a.next())
                .ok_or_else(|| DriverError::Connect {
                    message: format!("{} not found", t.first_host()),
                })?;
            builder
                .known_node_addr(at)
                .address_translator(Arc::new(ThroughTunnel(at)))
        }
        None => builder.known_nodes(t.hosts().map(|h| {
            if h.contains(':') && !h.starts_with('[') {
                h.to_owned()
            } else {
                format!("{h}:{}", t.port)
            }
        })),
    };
    if !t.user.is_empty() {
        builder = builder.user(t.user.clone(), password.unwrap_or_default());
    }
    if let Some(cfg) = tls_config(t, key_passphrase)? {
        builder = builder.tls_context(Some(cfg));
    }
    let dc = t.datacenter.trim();
    let policy = if dc.is_empty() {
        DefaultPolicy::builder().build()
    } else {
        DefaultPolicy::builder()
            .prefer_datacenter(dc.to_owned())
            .permit_dc_failover(true)
            .build()
    };
    let profile = ExecutionProfile::builder()
        .load_balancing_policy(policy)
        .consistency(level(t.consistency))
        .request_timeout(None)
        .build();
    builder = builder.default_execution_profile_handle(profile.into_handle());
    let database = t.database.trim();
    if !database.is_empty() {
        builder = builder.use_keyspace(database, true);
    }
    let session = builder.build().await.map_err(|e| {
        let message = e.to_string();
        if message.contains("uthenticat") || message.contains("credentials") {
            DriverError::Auth { message }
        } else {
            DriverError::Connect { message }
        }
    })?;
    let mut conn = CassandraConnection {
        session,
        cancel: Arc::new(Cancel {
            stop: Notify::new(),
        }),
        server: ServerInfo {
            product: "Cassandra".into(),
            version: String::new(),
            database: database.to_owned(),
        },
        consistency: Mutex::new(t.consistency),
        keys: Mutex::new(HashMap::new()),
    };
    if let Ok(rows) = conn.rows("SELECT version FROM system.versions", &[]).await
        && let Some(v) = rows.first().and_then(|r| text(r, 0))
    {
        conn.server.product = "ScyllaDB".into();
        conn.server.version = v;
    } else if let Ok(rows) = conn
        .rows("SELECT release_version FROM system.local", &[])
        .await
    {
        conn.server.version = rows.first().and_then(|r| text(r, 0)).unwrap_or_default();
    }
    Ok(conn)
}

fn text(row: &Row, i: usize) -> Option<String> {
    match row.columns.get(i)?.as_ref()? {
        CqlValue::Text(s) | CqlValue::Ascii(s) => Some(s.clone()),
        other => Some(match value::to_json(other) {
            Value::String(s) => s,
            v => v.to_string(),
        }),
    }
}

fn int(row: &Row, i: usize) -> i32 {
    match row.columns.get(i) {
        Some(Some(CqlValue::Int(n))) => *n,
        _ => 0,
    }
}

impl CassandraConnection {
    fn statement(&self, text: &str) -> Statement {
        let mut st = Statement::new(text.to_owned());
        st.set_page_size(PAGE);
        let c = self
            .consistency
            .lock()
            .map_or(Consistency::LocalQuorum, |c| *c);
        st.set_consistency(level(c));
        st
    }

    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let stop = self.cancel.stop.notified();
        tokio::select! {
            r = work => r,
            () = stop => Err(DriverError::Cancelled),
        }
    }

    async fn rows(&self, cql: &str, values: &[&str]) -> Result<Vec<Row>> {
        let mut st = Statement::new(cql.to_owned());
        st.set_consistency(scylla::statement::Consistency::One);
        let result = self
            .session
            .query_unpaged(st, values.to_vec())
            .await
            .map_err(query_err)?;
        let rows = result.into_rows_result().map_err(query_err)?;
        rows.rows::<Row>()
            .map_err(query_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(query_err)
    }

    async fn stream(&self, text: &str, out: &mpsc::Sender<Chunk>) -> Result<Outcome> {
        let st = self.statement(text);
        let mut paging = PagingState::start();
        loop {
            let (result, next) = self
                .session
                .query_single_page(st.clone(), (), paging)
                .await
                .map_err(query_err)?;
            if !result.is_rows() {
                return Ok(Outcome {
                    command: text.split_whitespace().next().map(str::to_uppercase),
                    rows_affected: None,
                });
            }
            let rows = result.into_rows_result().map_err(query_err)?;
            let names: Vec<String> = rows
                .column_specs()
                .iter()
                .map(|c| c.name().to_owned())
                .collect();
            let docs: Vec<String> = rows
                .rows::<Row>()
                .map_err(query_err)?
                .map(|r| r.map(|r| value::row_document(&names, &r)))
                .collect::<std::result::Result<_, _>>()
                .map_err(query_err)?;
            if !docs.is_empty() {
                out.send(Chunk::Documents(docs))
                    .await
                    .map_err(|_| DriverError::Cancelled)?;
            }
            match next {
                PagingStateResponse::HasMorePages { state } => paging = state,
                PagingStateResponse::NoMorePages => break,
            }
        }
        Ok(Outcome {
            command: Some("SELECT".into()),
            rows_affected: None,
        })
    }

    async fn keys_of(&self, keyspace: &str, table: &str) -> Result<Vec<String>> {
        let id = (keyspace.to_owned(), table.to_owned());
        if let Some(k) = self.keys.lock().ok().and_then(|m| m.get(&id).cloned()) {
            return Ok(k);
        }
        let rows = self
            .rows(
                "SELECT column_name, kind, position FROM system_schema.columns WHERE keyspace_name = ? AND table_name = ?",
                &[keyspace, table],
            )
            .await?;
        let cols: Vec<schema::Col> = rows
            .iter()
            .map(|r| schema::Col {
                table: table.to_owned(),
                name: text(r, 0).unwrap_or_default(),
                kind: text(r, 1).unwrap_or_default(),
                position: int(r, 2),
                type_name: String::new(),
                order: String::new(),
            })
            .collect();
        let (info, _) = schema::columns_of(cols.iter().collect());
        let keys: Vec<String> = info
            .into_iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name)
            .collect();
        if keys.is_empty() {
            return Err(DriverError::query(format!("{keyspace}.{table} not found")));
        }
        if let Ok(mut m) = self.keys.lock() {
            m.insert(id, keys.clone());
        }
        Ok(keys)
    }

    fn key_clause(
        keys: &[String],
        id: &serde_json::Map<String, Value>,
    ) -> Result<(String, Vec<String>)> {
        let mut parts = Vec::new();
        let mut values = Vec::new();
        for k in keys {
            let v = id
                .get(k)
                .ok_or_else(|| DriverError::query(format!("the key column {k} is missing")))?;
            parts.push(format!("{} = fromJson(?)", quote(k)));
            values.push(v.to_string());
        }
        Ok((parts.join(" AND "), values))
    }

    async fn exists(
        &self,
        table: &str,
        keys: &[String],
        id: &serde_json::Map<String, Value>,
    ) -> Result<bool> {
        let (clause, values) = Self::key_clause(keys, id)?;
        let refs: Vec<&str> = values.iter().map(String::as_str).collect();
        let cql = format!("SELECT {} FROM {table} WHERE {clause}", quote(&keys[0]));
        Ok(!self.rows(&cql, &refs).await?.is_empty())
    }

    async fn run_bound(&self, cql: &str, values: Vec<String>) -> Result<()> {
        let mut st = Statement::new(cql.to_owned());
        let c = self
            .consistency
            .lock()
            .map_or(Consistency::LocalQuorum, |c| *c);
        st.set_consistency(level(c));
        self.session
            .query_unpaged(st, values)
            .await
            .map(|_| ())
            .map_err(query_err)
    }

    async fn trace(&self, command: &str) -> Result<String> {
        let mut st = self.statement(command);
        st.set_tracing(true);
        let (result, _) = self
            .session
            .query_single_page(st, (), PagingState::start())
            .await
            .map_err(query_err)?;
        let id = result
            .tracing_id()
            .ok_or_else(|| DriverError::query("the server sent no trace"))?;
        let info = self
            .session
            .get_tracing_info(&id)
            .await
            .map_err(query_err)?;
        let events: Vec<Value> = info
            .events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "activity": e.activity,
                    "source": e.source.map(|s| s.to_string()),
                    "elapsed_us": e.source_elapsed,
                    "thread": e.thread,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "request": info.request,
            "command": info.command,
            "coordinator": info.coordinator.map(|c| c.to_string()),
            "duration_us": info.duration,
            "events": events,
        })
        .to_string())
    }

    fn table_ref(&self, database: &str, collection: &str) -> String {
        let ks = if database.is_empty() {
            self.server.database.as_str()
        } else {
            database
        };
        format!("{}.{}", quote(ks), quote(collection))
    }

    fn keyspace<'a>(&'a self, database: &'a str) -> &'a str {
        if database.is_empty() {
            &self.server.database
        } else {
            database
        }
    }
}

fn object(text: &str) -> Result<serde_json::Map<String, Value>> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(o)) => Ok(o),
        Ok(_) => Err(DriverError::query("a row is a JSON object")),
        Err(e) => Err(DriverError::query(format!("not valid JSON: {e}"))),
    }
}

#[async_trait::async_trait]
impl nd_driver::Connection for CassandraConnection {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(CassandraCanceller(self.cancel.clone()))
    }

    async fn execute(&self, text: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let text = text.trim().trim_end_matches(';').trim();
        if let Some(name) = cql::consistency_command(text) {
            if !name.is_empty() {
                let c = Consistency::parse(name).ok_or_else(|| {
                    DriverError::query(format!(
                        "{name} is not a consistency level: {}",
                        Consistency::ALL.map(Consistency::name).join(", ")
                    ))
                })?;
                if let Ok(mut cur) = self.consistency.lock() {
                    *cur = c;
                }
            }
            let now = self
                .consistency
                .lock()
                .map_or(Consistency::LocalQuorum, |c| *c);
            out.send(Chunk::Documents(vec![
                serde_json::json!({ "consistency": now.name() }).to_string(),
            ]))
            .await
            .map_err(|_| DriverError::Cancelled)?;
            return Ok(Outcome {
                command: Some("CONSISTENCY".into()),
                rows_affected: None,
            });
        }
        self.cancellable(self.stream(text, &out)).await
    }

    async fn execute_bound(
        &self,
        text: &str,
        _params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        self.execute(text, out).await
    }

    #[allow(clippy::too_many_lines)]
    async fn schema(&self) -> Result<Schema> {
        let ks_rows = self
            .rows("SELECT keyspace_name FROM system_schema.keyspaces", &[])
            .await?;
        let mut databases: Vec<String> = ks_rows
            .iter()
            .filter_map(|r| text(r, 0))
            .filter(|k| !k.starts_with("system") || *k == self.server.database)
            .collect();
        databases.sort();
        let ks = self.server.database.clone();
        let mut namespaces = Vec::new();
        if !ks.is_empty() {
            let k = ks.as_str();
            let tables = self
                .rows(
                    "SELECT table_name FROM system_schema.tables WHERE keyspace_name = ?",
                    &[k],
                )
                .await?;
            let views = self
                .rows("SELECT view_name, base_table_name FROM system_schema.views WHERE keyspace_name = ?", &[k])
                .await
                .unwrap_or_default();
            let columns = self
                .rows(
                    "SELECT table_name, column_name, kind, position, type, clustering_order FROM system_schema.columns WHERE keyspace_name = ?",
                    &[k],
                )
                .await?;
            let indexes = self
                .rows("SELECT table_name, index_name, options FROM system_schema.indexes WHERE keyspace_name = ?", &[k])
                .await
                .unwrap_or_default();
            let types = self
                .rows(
                    "SELECT type_name FROM system_schema.types WHERE keyspace_name = ?",
                    &[k],
                )
                .await
                .unwrap_or_default();
            let view_names: Vec<(String, String)> = views
                .iter()
                .filter_map(|r| Some((text(r, 0)?, text(r, 1).unwrap_or_default())))
                .collect();
            let catalog = schema::Catalog {
                keyspace: ks.clone(),
                tables: tables
                    .iter()
                    .filter_map(|r| text(r, 0))
                    .filter(|t| !view_names.iter().any(|(v, _)| v == t))
                    .collect(),
                views: view_names,
                columns: columns
                    .iter()
                    .map(|r| schema::Col {
                        table: text(r, 0).unwrap_or_default(),
                        name: text(r, 1).unwrap_or_default(),
                        kind: text(r, 2).unwrap_or_default(),
                        position: int(r, 3),
                        type_name: text(r, 4).unwrap_or_default(),
                        order: text(r, 5).unwrap_or_default(),
                    })
                    .collect(),
                indexes: indexes
                    .iter()
                    .filter_map(|r| {
                        let target = match r.columns.get(2) {
                            Some(Some(CqlValue::Map(m))) => {
                                m.iter().find_map(|(k, v)| match (k, v) {
                                    (CqlValue::Text(k), CqlValue::Text(v)) if k == "target" => {
                                        Some(schema::index_target(v))
                                    }
                                    _ => None,
                                })
                            }
                            _ => None,
                        };
                        Some((text(r, 0)?, text(r, 1)?, target.unwrap_or_default()))
                    })
                    .collect(),
                types: types.iter().filter_map(|r| text(r, 0)).collect(),
            };
            namespaces.push(schema::namespace(&catalog));
        }
        Ok(Schema {
            databases,
            current_database: ks,
            namespaces,
        })
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        let ks = self.keyspace(&object.namespace).to_owned();
        let what = match object.kind {
            DdlKind::View => "MATERIALIZED VIEW",
            DdlKind::Index => "INDEX",
            DdlKind::Sequence | DdlKind::Trigger | DdlKind::Routine => "TYPE",
            DdlKind::Table => "TABLE",
        };
        let cql = format!("DESCRIBE {what} {}.{}", quote(&ks), quote(&object.name));
        let rows = self.rows(&cql, &[]).await?;
        let script: Vec<String> = rows
            .iter()
            .filter_map(|r| text(r, 3).or_else(|| text(r, 0)))
            .collect();
        if script.is_empty() {
            return Err(object.not_found());
        }
        Ok(script.join("\n\n"))
    }

    async fn dependencies(&self, _object: &ObjectRef) -> Result<Vec<Dependency>> {
        Ok(Vec::new())
    }

    fn commands(&self, text: &str) -> Commands {
        cql::commands(text)
    }

    fn classify(&self, command: &str) -> CommandClass {
        cql::classify(command)
    }

    async fn ping(&self) -> Result<()> {
        self.rows("SELECT now() FROM system.local", &[])
            .await
            .map(|_| ())
    }

    async fn write_documents(&self, write: &DocumentWrite) -> Result<Outcome> {
        let (database, collection) = write.collection();
        let ks = self.keyspace(database).to_owned();
        let keys = self.keys_of(&ks, collection).await?;
        let table = self.table_ref(&ks, collection);
        let mut n = 0;
        match write {
            DocumentWrite::Insert { documents, .. } => {
                for d in documents {
                    let doc = object(d)?;
                    if self.exists(&table, &keys, &doc).await? {
                        return Err(DriverError::query(
                            "a row with this primary key already exists",
                        ));
                    }
                    self.run_bound(&format!("INSERT INTO {table} JSON ?"), vec![json_row(doc)])
                        .await?;
                    n += 1;
                }
            }
            DocumentWrite::Replace { id, document, .. } => {
                let id = object(id)?;
                let mut doc = object(document)?;
                if !self.exists(&table, &keys, &id).await? {
                    return Err(DriverError::query("the row is no longer there"));
                }
                for k in &keys {
                    if let Some(v) = id.get(k) {
                        doc.insert(k.clone(), v.clone());
                    }
                }
                self.run_bound(&format!("INSERT INTO {table} JSON ?"), vec![json_row(doc)])
                    .await?;
                n += 1;
            }
            DocumentWrite::Delete { ids, .. } => {
                for id in ids {
                    let id = object(id)?;
                    if !self.exists(&table, &keys, &id).await? {
                        return Err(DriverError::query("the row is no longer there"));
                    }
                    let (clause, values) = Self::key_clause(&keys, &id)?;
                    self.run_bound(&format!("DELETE FROM {table} WHERE {clause}"), values)
                        .await?;
                    n += 1;
                }
            }
        }
        Ok(Outcome {
            command: Some("write".into()),
            rows_affected: Some(n),
        })
    }

    fn describe_documents(&self, write: &DocumentWrite) -> String {
        let (database, collection) = write.collection();
        let table = self.table_ref(database, collection);
        let insert = |d: &str| format!("INSERT INTO {table} JSON {};", literal(d));
        match write {
            DocumentWrite::Insert { documents, .. } => documents
                .iter()
                .map(|d| object(d).map_or_else(|_| insert(d), |o| insert(&json_row(o))))
                .collect::<Vec<_>>()
                .join("\n"),
            DocumentWrite::Replace { id, document, .. } => {
                let mut doc = object(document).unwrap_or_default();
                doc.extend(object(id).unwrap_or_default());
                insert(&json_row(doc))
            }
            DocumentWrite::Delete { ids, .. } => ids
                .iter()
                .map(|id| {
                    let clause = object(id)
                        .unwrap_or_default()
                        .iter()
                        .map(|(k, v)| {
                            format!("{} = fromJson({})", quote(k), literal(&v.to_string()))
                        })
                        .collect::<Vec<_>>()
                        .join(" AND ");
                    format!("DELETE FROM {table} WHERE {clause};")
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    async fn explain_native(&self, command: &str) -> Result<String> {
        if cql::classify(command).write {
            return Err(DriverError::query(
                "a trace runs the statement: only reads are traced",
            ));
        }
        self.cancellable(self.trace(command.trim().trim_end_matches(';')))
            .await
    }

    #[allow(clippy::too_many_lines)]
    async fn report(&self) -> Result<Report> {
        let local = self
            .rows("SELECT cluster_name, release_version, partitioner, data_center, rack, broadcast_address, host_id, tokens, cql_version FROM system.local", &[])
            .await?;
        let peers = self
            .rows("SELECT peer, data_center, rack, release_version, host_id, tokens FROM system.peers", &[])
            .await
            .unwrap_or_default();
        let consistency = self
            .consistency
            .lock()
            .map_or(Consistency::LocalQuorum, |c| *c);
        let mut info = vec![
            vec!["product".into(), self.server.product.clone()],
            vec!["version".into(), self.server.version.clone()],
            vec!["consistency".into(), consistency.name().into()],
        ];
        let mut nodes = Vec::new();
        let mut ring: Vec<(i64, String, String)> = Vec::new();
        let tokens_of = |r: &Row, i: usize| -> Vec<i64> {
            match r.columns.get(i) {
                Some(Some(CqlValue::Set(s) | CqlValue::List(s))) => s
                    .iter()
                    .filter_map(|t| match t {
                        CqlValue::Text(t) => t.parse().ok(),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        };
        if let Some(l) = local.first() {
            for (i, name) in [(0, "cluster_name"), (2, "partitioner"), (8, "cql_version")] {
                info.push(vec![name.into(), text(l, i).unwrap_or_default()]);
            }
            let address = text(l, 5).unwrap_or_default();
            let tokens = tokens_of(l, 7);
            ring.extend(
                tokens
                    .iter()
                    .map(|t| (*t, address.clone(), text(l, 3).unwrap_or_default())),
            );
            nodes.push(vec![
                address,
                text(l, 3).unwrap_or_default(),
                text(l, 4).unwrap_or_default(),
                text(l, 1).unwrap_or_default(),
                tokens.len().to_string(),
                text(l, 6).unwrap_or_default(),
            ]);
        }
        for p in &peers {
            let address = text(p, 0).unwrap_or_default();
            let tokens = tokens_of(p, 5);
            ring.extend(
                tokens
                    .iter()
                    .map(|t| (*t, address.clone(), text(p, 1).unwrap_or_default())),
            );
            nodes.push(vec![
                address,
                text(p, 1).unwrap_or_default(),
                text(p, 2).unwrap_or_default(),
                text(p, 3).unwrap_or_default(),
                tokens.len().to_string(),
                text(p, 4).unwrap_or_default(),
            ]);
        }
        info.push(vec!["nodes".into(), nodes.len().to_string()]);
        ring.sort_by_key(|r| r.0);
        let ring_rows: Vec<Vec<String>> = ring
            .iter()
            .enumerate()
            .map(|(i, (t, node, dc))| {
                let next = ring
                    .get(i + 1)
                    .map_or_else(|| ring.first().map_or(*t, |f| f.0), |n| n.0);
                vec![t.to_string(), next.to_string(), node.clone(), dc.clone()]
            })
            .collect();
        let keyspaces = self
            .rows(
                "SELECT keyspace_name, replication, durable_writes FROM system_schema.keyspaces",
                &[],
            )
            .await
            .unwrap_or_default()
            .iter()
            .map(|r| {
                vec![
                    text(r, 0).unwrap_or_default(),
                    text(r, 1).unwrap_or_default(),
                    text(r, 2).unwrap_or_default(),
                ]
            })
            .collect();
        let grid = |id: &str, columns: &[&str], rows: Vec<Vec<String>>| ReportSection {
            id: id.into(),
            group: ReportGroup::Grid,
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            rows,
        };
        Ok(Report {
            sections: vec![
                ReportSection {
                    id: "cluster".into(),
                    group: ReportGroup::Info,
                    columns: vec!["name".into(), "value".into()],
                    rows: info,
                },
                grid(
                    "nodes",
                    &[
                        "address",
                        "datacenter",
                        "rack",
                        "version",
                        "tokens",
                        "host_id",
                    ],
                    nodes,
                ),
                grid(
                    "ring",
                    &["token", "next_token", "address", "datacenter"],
                    ring_rows,
                ),
                grid(
                    "keyspaces",
                    &["keyspace", "replication", "durable_writes"],
                    keyspaces,
                ),
            ],
        })
    }
}
