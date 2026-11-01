mod console;
mod reply;
mod schema;

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use bytes::Bytes;
use nd_driver::{
    Canceller, Capabilities, Chunk, Column, CommandClass, Commands, DdlKind, Dependency,
    DocumentWrite, DriverError, ElasticTarget, Family, Group, Language, NamespaceLevel, Noun,
    ObjectRef, Outcome, Report, ReportGroup, ReportSection, Result, ResultShape, Row, Schema,
    SchemaKind, ServerInfo, Tool, TreeShape, Vocabulary,
};
use nd_http::{Http, encode};
use serde_json::Value;
use tokio::sync::{Notify, mpsc};

use crate::console::{Command, Request};
use crate::reply::Shaped;

const REPLY_CAP: usize = 256 * 1024 * 1024;
const BATCH: usize = 4000;

pub fn capabilities() -> Capabilities {
    Capabilities {
        family: Family::Search,
        language: Language::Search,
        schema: SchemaKind::Inferred,
        results: ResultShape::Documents,
        tree: TreeShape {
            databases: false,
            namespaces: NamespaceLevel::Multiple,
            groups: vec![Group::Tables, Group::Aliases, Group::DataStreams],
            server: false,
        },
        words: Vocabulary {
            table: Noun::Index,
            column: Noun::Field,
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

struct Client {
    http: Http,
    auth: Option<String>,
    opaque: AtomicU64,
}

impl Client {
    async fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        tag: Option<&str>,
    ) -> Result<(u16, String)> {
        let ndjson = path
            .split('?')
            .next()
            .is_some_and(|p| p.ends_with("_bulk") || p.ends_with("_msearch"));
        let mut headers: Vec<(&str, &str)> = vec![(
            "Content-Type",
            if ndjson {
                "application/x-ndjson"
            } else {
                "application/json"
            },
        )];
        if let Some(a) = &self.auth {
            headers.push(("Authorization", a));
        }
        if let Some(t) = tag {
            headers.push(("X-Opaque-Id", t));
        }
        let mut text = body.unwrap_or_default().to_owned();
        if ndjson && !text.ends_with('\n') {
            text.push('\n');
        }
        let reply = self
            .http
            .send(method, path, &headers, Bytes::from(text))
            .await?;
        let status = reply.status;
        Ok((status, reply.text(REPLY_CAP).await?))
    }

    async fn json(&self, method: &str, path: &str, body: Option<&str>) -> Result<Value> {
        let (status, text) = self.call(method, path, body, None).await?;
        if status >= 400 {
            return Err(reply::error(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| DriverError::query(format!("{path}: {e}")))
    }
}

struct Cancel {
    stop: Notify,
    current: Mutex<Option<String>>,
    client: Arc<Client>,
}

struct ElasticCanceller(Arc<Cancel>);

#[async_trait::async_trait]
impl Canceller for ElasticCanceller {
    async fn cancel(&self) {
        let tag = self.0.current.lock().ok().and_then(|t| t.clone());
        self.0.stop.notify_waiters();
        if let Some(tag) = tag {
            cancel_tasks(&self.0.client, &tag).await;
        }
    }
}

async fn cancel_tasks(client: &Client, tag: &str) {
    let Ok(tasks) = client.json("GET", "/_tasks?detailed=true", None).await else {
        return;
    };
    for (node, n) in tasks
        .get("nodes")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        for (id, task) in n
            .get("tasks")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let mine = task
                .get("headers")
                .and_then(|h| h.get("X-Opaque-Id"))
                .and_then(Value::as_str)
                == Some(tag);
            if mine && task.get("cancellable").and_then(Value::as_bool) == Some(true) {
                let id = if id.contains(':') {
                    id.clone()
                } else {
                    format!("{node}:{id}")
                };
                let _ = client
                    .call(
                        "POST",
                        &format!("/_tasks/{}/_cancel", encode(&id)),
                        None,
                        None,
                    )
                    .await;
            }
        }
    }
}

pub struct ElasticConnection {
    client: Arc<Client>,
    cancel: Arc<Cancel>,
    server: ServerInfo,
    opensearch: bool,
    patterns: String,
}

fn authorization(user: &str, password: Option<&str>) -> Option<String> {
    let password = password.unwrap_or_default();
    if user.is_empty() {
        return (!password.is_empty()).then(|| format!("ApiKey {password}"));
    }
    let raw = format!("{user}:{password}");
    Some(format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    ))
}

pub async fn connect(
    t: &ElasticTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(IpAddr, u16)>,
) -> Result<ElasticConnection> {
    let identity = match (&t.client_cert, t.tls) {
        (Some(c), mode) if mode != nd_driver::TlsMode::Disable => {
            Some(nd_driver::load_identity(c, key_passphrase)?)
        }
        _ => None,
    };
    let tls = nd_http::tls_config(
        t.tls,
        t.ca_file.as_deref().filter(|f| !f.is_empty()),
        identity,
    )?;
    let auth = authorization(&t.user, password);
    let client = Arc::new(Client {
        http: Http::new(&t.host, t.port, via, tls),
        auth,
        opaque: AtomicU64::new(0),
    });
    let (status, text) = client.call("GET", "/", None, None).await?;
    if status >= 400 {
        return Err(match reply::error(status, &text) {
            DriverError::Query { message, .. } => DriverError::Connect { message },
            other => other,
        });
    }
    let root: Value = serde_json::from_str(&text).map_err(|_| DriverError::Connect {
        message: format!(
            "{}:{} does not answer as Elasticsearch or OpenSearch",
            t.host, t.port
        ),
    })?;
    let version = root.get("version");
    let opensearch = version
        .and_then(|v| v.get("distribution"))
        .and_then(Value::as_str)
        == Some("opensearch");
    let server = ServerInfo {
        product: if opensearch {
            "OpenSearch"
        } else {
            "Elasticsearch"
        }
        .into(),
        version: version
            .and_then(|v| v.get("number"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        database: root
            .get("cluster_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    };
    Ok(ElasticConnection {
        cancel: Arc::new(Cancel {
            stop: Notify::new(),
            current: Mutex::new(None),
            client: client.clone(),
        }),
        client,
        server,
        opensearch,
        patterns: t.database.clone(),
    })
}

fn path_with_json(path: &str) -> String {
    let cat = path.trim_start_matches('/').starts_with("_cat");
    if cat && !path.contains("format=") {
        let sep = if path.contains('?') { '&' } else { '?' };
        format!("{path}{sep}format=json")
    } else {
        path.to_owned()
    }
}

async fn send_shaped(out: &mpsc::Sender<Chunk>, shaped: Shaped) -> Result<u64> {
    let closed = |_| DriverError::Cancelled;
    match shaped {
        Shaped::Documents(docs) => {
            let n = docs.len() as u64;
            let mut docs = docs.into_iter().peekable();
            while docs.peek().is_some() {
                let part: Vec<String> = docs.by_ref().take(BATCH).collect();
                out.send(Chunk::Documents(part)).await.map_err(closed)?;
            }
            Ok(n)
        }
        Shaped::Grid(columns, rows) => {
            let n = rows.len() as u64;
            out.send(Chunk::Columns(columns)).await.map_err(closed)?;
            let mut rows = rows.into_iter().peekable();
            while rows.peek().is_some() {
                let part: Vec<Row> = rows.by_ref().take(BATCH).collect();
                out.send(Chunk::Rows(part)).await.map_err(closed)?;
            }
            Ok(n)
        }
    }
}

fn id_text(id: &str) -> Result<String> {
    match serde_json::from_str::<Value>(id) {
        Ok(Value::String(s)) => Ok(s),
        Ok(Value::Number(n)) => Ok(n.to_string()),
        Ok(Value::Object(o)) => o
            .get("_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| DriverError::query("the document has no _id")),
        _ => Ok(id.trim().to_owned()),
    }
}

fn source_of(document: &str) -> Result<(Option<String>, String)> {
    let mut doc: serde_json::Map<String, Value> = serde_json::from_str(document)
        .map_err(|e| DriverError::query(format!("not a JSON object: {e}")))?;
    let id = doc.remove("_id").and_then(|v| match v {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    });
    for k in [
        "_index",
        "_score",
        "_routing",
        "_seq_no",
        "_primary_term",
        "_version",
    ] {
        doc.remove(k);
    }
    Ok((id, Value::Object(doc).to_string()))
}

impl ElasticConnection {
    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let stop = self.cancel.stop.notified();
        tokio::select! {
            r = work => r,
            () = stop => Err(DriverError::Cancelled),
        }
    }

    fn tag(&self) -> String {
        let n = self.client.opaque.fetch_add(1, Ordering::Relaxed) + 1;
        let tag = format!("nicedb-{n}");
        if let Ok(mut c) = self.cancel.current.lock() {
            *c = Some(tag.clone());
        }
        tag
    }

    async fn request(
        &self,
        r: &Request<'_>,
        write: bool,
        out: &mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let tag = self.tag();
        let path = path_with_json(&r.path);
        let (status, text) = self
            .cancellable(self.client.call(&r.method, &path, r.body, Some(&tag)))
            .await?;
        if r.method == "HEAD" {
            let doc = serde_json::json!({ "status": status }).to_string();
            send_shaped(out, Shaped::Documents(vec![doc])).await?;
            return Ok(Outcome {
                command: Some("HEAD".into()),
                rows_affected: None,
            });
        }
        if status >= 400 {
            return Err(reply::error(status, &text));
        }
        let shaped = reply::shape(&text)?;
        send_shaped(out, shaped).await?;
        Ok(Outcome {
            command: Some(r.method.clone()),
            rows_affected: if write { reply::affected(&text) } else { None },
        })
    }

    async fn sql(&self, sql: &str, out: &mpsc::Sender<Chunk>) -> Result<Outcome> {
        let tag = self.tag();
        let path = if self.opensearch {
            "/_plugins/_sql?format=jdbc"
        } else {
            "/_sql?format=json"
        };
        let mut body = serde_json::json!({ "query": sql, "fetch_size": 1000 }).to_string();
        let mut sent_columns = false;
        loop {
            let (status, text) = self
                .cancellable(self.client.call("POST", path, Some(&body), Some(&tag)))
                .await?;
            if status >= 400 {
                return Err(reply::error(status, &text));
            }
            let v: Value =
                serde_json::from_str(&text).map_err(|e| DriverError::query(e.to_string()))?;
            if !sent_columns {
                let cols = v
                    .get("schema")
                    .or_else(|| v.get("columns"))
                    .and_then(Value::as_array);
                let columns = cols
                    .into_iter()
                    .flatten()
                    .map(|c| Column {
                        name: c
                            .get("alias")
                            .or_else(|| c.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        type_name: c.get("type").and_then(Value::as_str).map(str::to_owned),
                    })
                    .collect();
                out.send(Chunk::Columns(columns))
                    .await
                    .map_err(|_| DriverError::Cancelled)?;
                sent_columns = true;
            }
            let rows: Vec<Row> = v
                .get("datarows")
                .or_else(|| v.get("rows"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|r| {
                    r.as_array()
                        .into_iter()
                        .flatten()
                        .map(reply::cell)
                        .collect()
                })
                .collect();
            if !rows.is_empty() {
                out.send(Chunk::Rows(rows))
                    .await
                    .map_err(|_| DriverError::Cancelled)?;
            }
            match v.get("cursor").and_then(Value::as_str) {
                Some(c) if !c.is_empty() => body = serde_json::json!({ "cursor": c }).to_string(),
                _ => break,
            }
        }
        Ok(Outcome {
            command: Some("SELECT".into()),
            rows_affected: None,
        })
    }

    async fn write_one(
        &self,
        index: &str,
        id: Option<&str>,
        source: &str,
        must_exist: bool,
    ) -> Result<u64> {
        let index = encode(index);
        if must_exist && let Some(id) = id {
            let (status, _) = self
                .client
                .call("HEAD", &format!("/{index}/_doc/{}", encode(id)), None, None)
                .await?;
            if status == 404 {
                return Err(DriverError::query(format!(
                    "document {id} is no longer there"
                )));
            }
        }
        let (method, path) = match id {
            Some(id) if must_exist => (
                "PUT",
                format!("/{index}/_doc/{}?refresh=wait_for", encode(id)),
            ),
            Some(id) => (
                "PUT",
                format!("/{index}/_create/{}?refresh=wait_for", encode(id)),
            ),
            None => ("POST", format!("/{index}/_doc?refresh=wait_for")),
        };
        let v = self.client.json(method, &path, Some(source)).await?;
        Ok(reply::affected(&v.to_string()).unwrap_or(1))
    }

    async fn grid(&self, path: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
        let (status, text) = self.client.call("GET", path, None, None).await?;
        if status >= 400 {
            return Err(reply::error(status, &text));
        }
        let items: Vec<reply::Ordered> =
            serde_json::from_str(&text).map_err(|e| DriverError::query(e.to_string()))?;
        let (cols, rows) = reply::table(&items);
        Ok((
            cols.into_iter().map(|c| c.name).collect(),
            rows.into_iter()
                .map(|r| r.into_iter().map(Option::unwrap_or_default).collect())
                .collect(),
        ))
    }
}

#[async_trait::async_trait]
impl nd_driver::Connection for ElasticConnection {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(ElasticCanceller(self.cancel.clone()))
    }

    async fn execute(&self, text: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        match console::parse(text) {
            Some(Command::Http(r)) => self.request(&r, console::classify(text).write, &out).await,
            Some(Command::Sql(sql)) => self.sql(sql, &out).await,
            None => Err(DriverError::query(
                "write a request as METHOD /path, with its JSON body on the lines below, or an SQL SELECT",
            )),
        }
    }

    async fn execute_bound(
        &self,
        text: &str,
        _params: &[Option<String>],
        out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        self.execute(text, out).await
    }

    async fn schema(&self) -> Result<Schema> {
        let c = &self.client;
        let (indices, mappings, aliases, streams) = tokio::join!(
            c.json("GET", "/_cat/indices?format=json&h=index,health,status,docs.count,store.size&expand_wildcards=open", None),
            c.json("GET", "/_mapping?expand_wildcards=open", None),
            c.json("GET", "/_alias", None),
            c.json("GET", "/_data_stream", None),
        );
        Ok(schema::build(&schema::Sources {
            cluster: self.server.database.clone(),
            patterns: self.patterns.clone(),
            indices: indices?,
            mappings: mappings?,
            aliases: aliases.unwrap_or(Value::Null),
            streams: streams.unwrap_or(Value::Null),
        }))
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        let name = encode(&object.name);
        if object.kind == DdlKind::View {
            let v = self
                .client
                .json("GET", &format!("/_alias/{name}"), None)
                .await?;
            let mut actions = Vec::new();
            for (index, a) in v.as_object().into_iter().flatten() {
                let mut add = serde_json::Map::new();
                add.insert("index".into(), Value::from(index.as_str()));
                add.insert("alias".into(), Value::from(object.name.as_str()));
                if let Some(def) = a
                    .get("aliases")
                    .and_then(|x| x.get(&object.name))
                    .and_then(Value::as_object)
                {
                    for (k, x) in def {
                        add.insert(k.clone(), x.clone());
                    }
                }
                actions.push(serde_json::json!({ "add": add }));
            }
            let body = serde_json::json!({ "actions": actions });
            return Ok(format!(
                "POST /_aliases\n{}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            ));
        }
        let v = self.client.json("GET", &format!("/{name}"), None).await?;
        let (real, def) = v
            .as_object()
            .and_then(|o| o.iter().next())
            .ok_or_else(|| object.not_found())?;
        let mut body = serde_json::Map::new();
        if let Some(s) = def.get("settings") {
            body.insert(
                "settings".into(),
                serde_json::json!({ "index": schema::creatable_settings(s) }),
            );
        }
        if let Some(m) = def.get("mappings") {
            body.insert("mappings".into(), m.clone());
        }
        if let Some(a) = def
            .get("aliases")
            .filter(|a| a.as_object().is_some_and(|o| !o.is_empty()))
        {
            body.insert("aliases".into(), a.clone());
        }
        Ok(format!(
            "PUT /{}\n{}",
            encode(real),
            serde_json::to_string_pretty(&Value::Object(body)).unwrap_or_default()
        ))
    }

    async fn dependencies(&self, _object: &ObjectRef) -> Result<Vec<Dependency>> {
        Ok(Vec::new())
    }

    fn commands(&self, text: &str) -> Commands {
        console::commands(text)
    }

    fn classify(&self, command: &str) -> CommandClass {
        console::classify(command)
    }

    async fn ping(&self) -> Result<()> {
        self.client.json("GET", "/", None).await.map(|_| ())
    }

    async fn write_documents(&self, write: &DocumentWrite) -> Result<Outcome> {
        let mut n = 0;
        match write {
            DocumentWrite::Insert {
                collection,
                documents,
                ..
            } => {
                for d in documents {
                    let (id, source) = source_of(d)?;
                    n += self
                        .write_one(collection, id.as_deref(), &source, false)
                        .await?;
                }
            }
            DocumentWrite::Replace {
                collection,
                id,
                document,
                ..
            } => {
                let (_, source) = source_of(document)?;
                n += self
                    .write_one(collection, Some(&id_text(id)?), &source, true)
                    .await?;
            }
            DocumentWrite::Delete {
                collection, ids, ..
            } => {
                for id in ids {
                    let id = id_text(id)?;
                    let path = format!(
                        "/{}/_doc/{}?refresh=wait_for",
                        encode(collection),
                        encode(&id)
                    );
                    let (status, text) = self.client.call("DELETE", &path, None, None).await?;
                    if status == 404 {
                        return Err(DriverError::query(format!(
                            "document {id} is no longer there"
                        )));
                    }
                    if status >= 400 {
                        return Err(reply::error(status, &text));
                    }
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
        let source = |d: &str| source_of(d).unwrap_or((None, d.to_owned()));
        match write {
            DocumentWrite::Insert {
                collection,
                documents,
                ..
            } => documents
                .iter()
                .map(|d| match source(d) {
                    (Some(id), s) => {
                        format!("PUT /{}/_create/{}\n{s}", encode(collection), encode(&id))
                    }
                    (None, s) => format!("POST /{}/_doc\n{s}", encode(collection)),
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            DocumentWrite::Replace {
                collection,
                id,
                document,
                ..
            } => format!(
                "PUT /{}/_doc/{}\n{}",
                encode(collection),
                encode(&id_text(id).unwrap_or_default()),
                source(document).1
            ),
            DocumentWrite::Delete {
                collection, ids, ..
            } => ids
                .iter()
                .map(|id| {
                    format!(
                        "DELETE /{}/_doc/{}",
                        encode(collection),
                        encode(&id_text(id).unwrap_or_default())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    async fn explain_native(&self, command: &str) -> Result<String> {
        let Some(Command::Http(r)) = console::parse(command) else {
            return Err(DriverError::query("only a _search request can be profiled"));
        };
        if !r.is_search() {
            return Err(DriverError::query("only a _search request can be profiled"));
        }
        let mut body: serde_json::Map<String, Value> = match r.body {
            Some(b) => serde_json::from_str(b)
                .map_err(|e| DriverError::query(format!("the body is not valid JSON: {e}")))?,
            None => serde_json::Map::new(),
        };
        body.insert("profile".into(), Value::Bool(true));
        let tag = self.tag();
        let (status, text) = self
            .cancellable(self.client.call(
                "POST",
                &r.path,
                Some(&Value::Object(body).to_string()),
                Some(&tag),
            ))
            .await?;
        if status >= 400 {
            return Err(reply::error(status, &text));
        }
        Ok(text)
    }

    async fn report(&self) -> Result<Report> {
        let health = self.client.json("GET", "/_cluster/health", None).await?;
        let mut info = vec![
            vec!["product".to_owned(), self.server.product.clone()],
            vec!["version".to_owned(), self.server.version.clone()],
        ];
        for (k, v) in health.as_object().into_iter().flatten() {
            info.push(vec![k.clone(), reply::cell(v).unwrap_or_default()]);
        }
        let mut sections = vec![ReportSection {
            id: "health".into(),
            group: ReportGroup::Info,
            columns: vec!["name".into(), "value".into()],
            rows: info,
        }];
        for (id, path) in [
            (
                "nodes",
                "/_cat/nodes?format=json&h=name,ip,node.role,master,heap.percent,ram.percent,cpu,load_1m,disk.used_percent,version",
            ),
            (
                "indices",
                "/_cat/indices?format=json&h=health,status,index,pri,rep,docs.count,store.size&s=index",
            ),
            (
                "shards",
                "/_cat/shards?format=json&h=index,shard,prirep,state,docs,store,node&s=index",
            ),
        ] {
            match self.grid(path).await {
                Ok((columns, mut rows)) => {
                    rows.truncate(5000);
                    sections.push(ReportSection {
                        id: id.into(),
                        group: ReportGroup::Grid,
                        columns,
                        rows,
                    });
                }
                Err(e) => tracing::debug!(error = %e, section = id, "cluster section skipped"),
            }
        }
        Ok(Report { sections })
    }
}
