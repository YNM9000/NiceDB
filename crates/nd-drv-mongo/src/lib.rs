#![forbid(unsafe_code)]

mod cert;
mod infer;
mod mql;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt, stream};
use mongodb::bson::{Bson, Document, doc};
use mongodb::options::{ClientOptions, Credential, ServerAddress, Tls, TlsOptions};
use mongodb::results::CollectionType;
use mongodb::{Client, ClientSession, Database};
use nd_driver::{
    Canceller, Capabilities, Chunk, CommandClass, Commands, DdlKind, Dependency, DocumentWrite,
    DriverError, Family, Group, Index, Language, MongoTarget, Namespace, NamespaceLevel, Noun,
    ObjectRef, Outcome, Relation, Result, ResultShape, Schema, SchemaKind, ServerInfo, TlsMode,
    Tool, TreeShape, Vocabulary,
};
use tokio::sync::{Notify, mpsc};

pub use mql::{classify, parse, split};

const SAMPLE: i64 = 100;
const SAMPLING: usize = 6;

const FIRST_BATCH: usize = 200;
const STEADY_BATCH: usize = 4000;

pub struct MongoConnection {
    client: Client,
    database: String,
    server: ServerInfo,
    cancel: Arc<Cancel>,
    transactions: bool,
    tx: tokio::sync::Mutex<Option<ClientSession>>,
    _cert: Option<cert::CertFile>,
}

#[derive(Default)]
struct Cancel {
    current: Mutex<u64>,
    stop: Notify,
    client: Mutex<Option<Client>>,
}

struct MongoCanceller(Arc<Cancel>);

#[async_trait::async_trait]
impl Canceller for MongoCanceller {
    async fn cancel(&self) {
        let tag = self.0.current.lock().map(|n| *n).unwrap_or_default();
        self.0.stop.notify_waiters();
        let client = self.0.client.lock().ok().and_then(|c| c.clone());
        if let Some(client) = client {
            kill(&client, &comment(tag)).await;
        }
    }
}

fn comment(tag: u64) -> String {
    format!("nicedb:{tag}")
}

async fn kill(client: &Client, comment: &str) {
    let admin = client.database("admin");
    let ops = admin
        .aggregate([
            doc! { "$currentOp": { "allUsers": true } },
            doc! { "$match": { "command.comment": comment } },
        ])
        .await;
    let Ok(mut ops) = ops else { return };
    while let Ok(Some(op)) = ops.try_next().await {
        if let Some(id) = op.get("opid") {
            let _ = admin
                .run_command(doc! { "killOp": 1, "op": id.clone() })
                .await;
        }
    }
}

fn driver_error(e: &mongodb::error::Error) -> DriverError {
    use mongodb::error::ErrorKind;
    let message = e.to_string();
    match e.kind.as_ref() {
        ErrorKind::Authentication { .. } => DriverError::Auth { message },
        ErrorKind::ServerSelection { .. } | ErrorKind::DnsResolve { .. } => {
            DriverError::Connect { message }
        }
        ErrorKind::Io(_) | ErrorKind::ConnectionPoolCleared { .. } => DriverError::Io { message },
        ErrorKind::Command(c) => DriverError::Query {
            message: c.message.clone(),
            code: Some(c.code_name.clone()).filter(|s| !s.is_empty()),
            detail: None,
            hint: None,
            offset: None,
        },
        ErrorKind::Write(_) | ErrorKind::InsertMany(_) | ErrorKind::BulkWrite(_) => {
            DriverError::query(message)
        }
        _ if message.to_ascii_lowercase().contains("tls")
            || message.to_ascii_lowercase().contains("certificate") =>
        {
            DriverError::Tls { message }
        }
        _ => DriverError::query(message),
    }
}

fn err(e: mongodb::error::Error) -> DriverError {
    driver_error(&e)
}

fn uri(t: &MongoTarget, via: Option<(std::net::IpAddr, u16)>) -> String {
    let host = match via {
        Some((ip, port)) => format!("{ip}:{port}"),
        None if t.srv => t.host.clone(),
        None => format!("{}:{}", t.host, t.port),
    };
    let scheme = if t.srv && via.is_none() {
        "mongodb+srv"
    } else {
        "mongodb"
    };
    let mut query: Vec<&str> = t
        .options
        .trim_start_matches('?')
        .split('&')
        .filter(|s| !s.is_empty())
        .collect();
    if via.is_some() {
        query.retain(|q| !q.starts_with("replicaSet=") && !q.starts_with("directConnection="));
        query.push("directConnection=true");
    }
    if query.is_empty() {
        format!("{scheme}://{host}/")
    } else {
        format!("{scheme}://{host}/?{}", query.join("&"))
    }
}

pub async fn connect(
    t: &MongoTarget,
    password: Option<&str>,
    key_passphrase: Option<&str>,
    via: Option<(std::net::IpAddr, u16)>,
) -> Result<MongoConnection> {
    let mut opts = ClientOptions::parse(uri(t, via))
        .await
        .map_err(|e| DriverError::Connect {
            message: e.to_string(),
        })?;
    if via.is_some() {
        opts.hosts = via
            .map(|(ip, port)| ServerAddress::Tcp {
                host: ip.to_string(),
                port: Some(port),
            })
            .into_iter()
            .collect();
    }
    if !t.user.is_empty() {
        opts.credential = Some(
            Credential::builder()
                .username(t.user.clone())
                .password(password.map(str::to_owned))
                .source(Some(t.auth_source.trim().to_owned()).filter(|s| !s.is_empty()))
                .build(),
        );
    }
    let identity = match (&t.client_cert, t.tls) {
        (Some(c), mode) if mode != TlsMode::Disable => {
            Some(cert::CertFile::new(c, key_passphrase)?)
        }
        _ => None,
    };
    opts.tls = Some(match t.tls {
        TlsMode::Disable => Tls::Disabled,
        mode => {
            let mut tls = TlsOptions::builder()
                .allow_invalid_certificates(mode == TlsMode::Insecure)
                .ca_file_path(t.ca_file.clone().filter(|f| !f.is_empty()).map(Into::into))
                .build();
            if let Some(id) = &identity {
                tls.cert_key_file_path = Some(id.path().to_owned());
                tls.tls_certificate_key_file_password = id.password();
            }
            Tls::Enabled(tls)
        }
    });
    opts.app_name = Some("NiceDB".into());
    opts.server_selection_timeout = Some(Duration::from_secs(10));
    opts.connect_timeout = Some(Duration::from_secs(10));
    let client = Client::with_options(opts).map_err(err)?;
    let info = client
        .database("admin")
        .run_command(doc! { "buildInfo": 1 })
        .await
        .map_err(err)?;
    let database = if t.database.is_empty() {
        default_database(&client.list_database_names().await.unwrap_or_default())
    } else {
        t.database.clone()
    };
    let server = ServerInfo {
        product: "MongoDB".into(),
        version: info.get_str("version").unwrap_or_default().to_owned(),
        database: database.clone(),
    };
    let transactions = client
        .database("admin")
        .run_command(doc! { "hello": 1 })
        .await
        .is_ok_and(|hello| supports_transactions(&hello));
    let cancel = Arc::new(Cancel::default());
    if let Ok(mut c) = cancel.client.lock() {
        *c = Some(client.clone());
    }
    Ok(MongoConnection {
        client,
        database,
        server,
        cancel,
        transactions,
        tx: tokio::sync::Mutex::new(None),
        _cert: identity,
    })
}

fn default_database(names: &[String]) -> String {
    names
        .iter()
        .find(|n| !matches!(n.as_str(), "admin" | "local" | "config"))
        .cloned()
        .unwrap_or_else(|| "test".into())
}

pub fn supports_transactions(hello: &Document) -> bool {
    hello.get_str("setName").is_ok_and(|s| !s.is_empty()) || hello.get_str("msg") == Ok("isdbgrid")
}

pub fn capabilities(transactions: bool) -> Capabilities {
    Capabilities {
        family: Family::Document,
        language: Language::Mql,
        schema: SchemaKind::Inferred,
        results: ResultShape::Documents,
        tree: TreeShape {
            databases: true,
            namespaces: NamespaceLevel::Multiple,
            groups: vec![Group::Tables, Group::Views],
            server: true,
        },
        words: Vocabulary {
            table: Noun::Collection,
            column: Noun::Field,
        },
        transactions,
        editable: true,
        explain: true,
        er: true,
        ddl: true,
        sort: false,
        params: false,
        tools: vec![Tool::Activity, Tool::Space, Tool::Users],
        cancel: nd_driver::Cancel::Server,
    }
}

pub fn json(doc: Document) -> String {
    Bson::Document(doc).into_relaxed_extjson().to_string()
}

fn pretty(doc: Document) -> String {
    serde_json::to_string_pretty(&Bson::Document(doc).into_relaxed_extjson()).unwrap_or_default()
}

fn value(text: &str) -> Result<Bson> {
    let v: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| DriverError::query(format!("not valid JSON: {e}")))?;
    Bson::try_from(v).map_err(|e| DriverError::query(format!("not valid extended JSON: {e}")))
}

fn document(text: &str) -> Result<Document> {
    match value(text)? {
        Bson::Document(d) => Ok(d),
        _ => Err(DriverError::query("a document is a JSON object")),
    }
}

impl MongoConnection {
    fn db(&self) -> Database {
        self.client.database(&self.database)
    }

    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let stop = self.cancel.stop.notified();
        tokio::select! {
            r = work => r,
            () = stop => Err(DriverError::Cancelled),
        }
    }

    fn db_for(&self, cmd: &mut Document) -> Database {
        match cmd.remove("$db") {
            Some(Bson::String(name)) if !name.is_empty() => self.client.database(&name),
            _ => self.db(),
        }
    }

    async fn stream(
        &self,
        db: Database,
        cmd: Document,
        out: &mpsc::Sender<Chunk>,
        tx: Option<&mut ClientSession>,
    ) -> Result<u64> {
        let mut batches = Batches::new(out);
        if let Some(session) = tx {
            let mut cursor = db
                .run_cursor_command(cmd)
                .session(&mut *session)
                .await
                .map_err(err)?;
            while let Some(doc) = cursor.next(&mut *session).await {
                batches.push(doc.map_err(err)?).await?;
            }
        } else {
            let mut cursor = db.run_cursor_command(cmd).await.map_err(err)?;
            while let Some(doc) = cursor.try_next().await.map_err(err)? {
                batches.push(doc).await?;
            }
        }
        batches.finish().await
    }

    async fn collection_schema(&self, name: String, view: bool) -> Relation {
        let coll = self.db().collection::<Document>(&name);
        let pipeline = if view {
            vec![doc! { "$limit": SAMPLE }]
        } else {
            vec![doc! { "$sample": { "size": SAMPLE } }]
        };
        let sample: Vec<Document> = match coll.aggregate(pipeline).await {
            Ok(c) => c.try_collect().await.unwrap_or_default(),
            Err(e) => {
                tracing::debug!(error = %e, "collection not sampled");
                Vec::new()
            }
        };
        let indexes = if view {
            Vec::new()
        } else {
            match coll.list_indexes().await {
                Ok(c) => c
                    .try_collect::<Vec<_>>()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| Index {
                        name: m
                            .options
                            .as_ref()
                            .and_then(|o| o.name.clone())
                            .unwrap_or_default(),
                        columns: m.keys.keys().cloned().collect(),
                        unique: m.options.as_ref().and_then(|o| o.unique).unwrap_or(false),
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        };
        let estimated_rows = if view {
            None
        } else {
            #[allow(clippy::cast_precision_loss)]
            coll.estimated_document_count().await.ok().map(|n| n as f64)
        };
        Relation {
            columns: infer::columns(&sample),
            name,
            indexes,
            foreign_keys: Vec::new(),
            estimated_rows,
            detail: None,
        }
    }
}

struct Batches<'a> {
    out: &'a mpsc::Sender<Chunk>,
    batch: Vec<String>,
    limit: usize,
    count: u64,
}

impl<'a> Batches<'a> {
    fn new(out: &'a mpsc::Sender<Chunk>) -> Self {
        Self {
            out,
            batch: Vec::with_capacity(FIRST_BATCH),
            limit: FIRST_BATCH,
            count: 0,
        }
    }

    async fn push(&mut self, doc: Document) -> Result<()> {
        self.batch.push(json(doc));
        self.count += 1;
        if self.batch.len() >= self.limit {
            self.limit = STEADY_BATCH;
            let full = std::mem::replace(&mut self.batch, Vec::with_capacity(STEADY_BATCH));
            self.out
                .send(Chunk::Documents(full))
                .await
                .map_err(|_| DriverError::Cancelled)?;
        }
        Ok(())
    }

    async fn finish(self) -> Result<u64> {
        if !self.batch.is_empty() {
            self.out
                .send(Chunk::Documents(self.batch))
                .await
                .map_err(|_| DriverError::Cancelled)?;
        }
        Ok(self.count)
    }
}

fn touched(name: &str, reply: &Document) -> Option<u64> {
    let key = match name {
        "insert" | "delete" => "n",
        "update" => "nModified",
        _ => return None,
    };
    match reply.get(key)? {
        Bson::Int32(n) => u64::try_from(*n).ok(),
        Bson::Int64(n) => u64::try_from(*n).ok(),
        _ => None,
    }
}

#[async_trait::async_trait]
impl nd_driver::Connection for MongoConnection {
    fn capabilities(&self) -> Capabilities {
        capabilities(self.transactions)
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(MongoCanceller(self.cancel.clone()))
    }

    async fn execute(&self, sql: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let mut cmd = mql::parse(sql)?;
        let name = mql::name(&cmd).to_owned();
        let tag = {
            let mut n = self
                .cancel
                .current
                .lock()
                .map_err(|_| DriverError::query("cancel state poisoned"))?;
            *n += 1;
            *n
        };
        if mql::CURSORS.contains(&name.as_str()) {
            if name == "aggregate" && !cmd.contains_key("cursor") {
                cmd.insert("cursor", Document::new());
            }
            if matches!(name.as_str(), "find" | "aggregate") && !cmd.contains_key("comment") {
                cmd.insert("comment", comment(tag));
            }
            let db = self.db_for(&mut cmd);
            let mut tx = self.tx.lock().await;
            self.cancellable(self.stream(db, cmd, &out, tx.as_mut()))
                .await?;
            return Ok(Outcome {
                command: Some(name),
                rows_affected: None,
            });
        }
        let db = self.db_for(&mut cmd);
        let mut tx = self.tx.lock().await;
        let reply = self
            .cancellable(async {
                match tx.as_mut() {
                    Some(session) => db.run_command(cmd).session(session).await,
                    None => db.run_command(cmd).await,
                }
                .map_err(err)
            })
            .await?;
        let rows_affected = touched(&name, &reply);
        out.send(Chunk::Documents(vec![json(reply)]))
            .await
            .map_err(|_| DriverError::Cancelled)?;
        Ok(Outcome {
            command: Some(name),
            rows_affected,
        })
    }

    async fn execute_bound(
        &self,
        _sql: &str,
        _params: &[Option<String>],
        _out: mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        Err(DriverError::query("MongoDB commands take no parameters"))
    }

    async fn schema(&self) -> Result<Schema> {
        let mut databases = self
            .client
            .list_database_names()
            .await
            .unwrap_or_else(|_| vec![self.database.clone()]);
        if !databases.contains(&self.database) {
            databases.push(self.database.clone());
        }
        databases.sort();
        let specs: Vec<_> = self
            .db()
            .list_collections()
            .await
            .map_err(err)?
            .try_collect()
            .await
            .map_err(err)?;
        let mut wanted: Vec<(String, bool)> = specs
            .into_iter()
            .filter(|s| !s.name.starts_with("system."))
            .map(|s| (s.name, s.collection_type == CollectionType::View))
            .collect();
        wanted.sort();
        let relations: Vec<(Relation, bool)> = stream::iter(wanted)
            .map(|(name, view)| async move { (self.collection_schema(name, view).await, view) })
            .buffered(SAMPLING)
            .collect()
            .await;
        let mut ns = Namespace::new(self.database.clone());
        for (rel, view) in relations {
            if view {
                ns.views.push(rel);
            } else {
                ns.tables.push(rel);
            }
        }
        Ok(Schema {
            databases,
            current_database: self.database.clone(),
            namespaces: vec![ns],
        })
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        let name = match object.kind {
            DdlKind::Index => object.table.clone().unwrap_or_default(),
            _ => object.name.clone(),
        };
        let spec = self
            .db()
            .list_collections()
            .filter(doc! { "name": &name })
            .await
            .map_err(err)?
            .try_next()
            .await
            .map_err(err)?
            .ok_or_else(|| DriverError::query(format!("no collection named {name}")))?;
        let mut create = doc! { "create": &name };
        let options = mongodb::bson::to_document(&spec.options).unwrap_or_default();
        for (k, v) in options {
            if !matches!(v, Bson::Null) {
                create.insert(k, v);
            }
        }
        if spec.collection_type == CollectionType::View {
            return Ok(pretty(create));
        }
        let indexes: Vec<Document> = self
            .db()
            .collection::<Document>(&name)
            .list_indexes()
            .await
            .map_err(err)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(err)?
            .into_iter()
            .filter_map(|m| mongodb::bson::to_document(&m).ok())
            .filter(|d| d.get_str("name").is_ok_and(|n| n != "_id_"))
            .filter(|d| object.kind != DdlKind::Index || d.get_str("name") == Ok(&object.name))
            .map(|mut d| {
                d.remove("v");
                d
            })
            .collect();
        let script = if indexes.is_empty() {
            None
        } else {
            Some(pretty(doc! { "createIndexes": &name, "indexes": indexes }))
        };
        Ok(match (object.kind, script) {
            (DdlKind::Index, Some(s)) => s,
            (DdlKind::Index, None) => {
                return Err(DriverError::query(format!(
                    "no index named {}",
                    object.name
                )));
            }
            (_, Some(s)) => format!("{}\n\n{s}", pretty(create)),
            (_, None) => pretty(create),
        })
    }

    async fn dependencies(&self, _object: &ObjectRef) -> Result<Vec<Dependency>> {
        Ok(Vec::new())
    }

    fn commands(&self, text: &str) -> Commands {
        mql::split(text)
    }

    fn classify(&self, command: &str) -> CommandClass {
        mql::classify(command)
    }

    async fn ping(&self) -> Result<()> {
        self.db()
            .run_command(doc! { "ping": 1 })
            .await
            .map(|_| ())
            .map_err(err)
    }

    async fn begin_native(&self) -> Result<()> {
        if !self.transactions {
            return Err(DriverError::Query {
                message: "this MongoDB server is standalone: transactions need a replica set or a sharded cluster".into(),
                code: Some("IllegalOperation".into()),
                detail: None,
                hint: Some("start mongod with --replSet, even a replica set of one member".into()),
                offset: None,
            });
        }
        let mut tx = self.tx.lock().await;
        if tx.is_some() {
            return Ok(());
        }
        let mut session = self.client.start_session().await.map_err(err)?;
        session.start_transaction().await.map_err(err)?;
        *tx = Some(session);
        Ok(())
    }

    async fn end_native(&self, commit: bool) -> Result<()> {
        let Some(mut session) = self.tx.lock().await.take() else {
            return Ok(());
        };
        if commit {
            session.commit_transaction().await.map_err(err)
        } else {
            session.abort_transaction().await.map_err(err)
        }
    }

    async fn write_documents(&self, write: &DocumentWrite) -> Result<Outcome> {
        let (database, collection) = write.collection();
        let coll = self
            .client
            .database(database)
            .collection::<Document>(collection);
        let (command, n) = match write {
            DocumentWrite::Insert { documents, .. } => {
                let docs = documents
                    .iter()
                    .map(|d| document(d))
                    .collect::<Result<Vec<_>>>()?;
                let r = coll.insert_many(docs).await.map_err(err)?;
                ("insert", r.inserted_ids.len() as u64)
            }
            DocumentWrite::Replace {
                id, document: d, ..
            } => {
                let mut d = document(d)?;
                let id = value(id)?;
                d.remove("_id");
                let r = coll.replace_one(doc! { "_id": id }, d).await.map_err(err)?;
                if r.matched_count == 0 {
                    return Err(DriverError::query(
                        "the document is gone: someone deleted it or changed its _id",
                    ));
                }
                ("update", r.modified_count)
            }
            DocumentWrite::Delete { ids, .. } => {
                let ids = ids.iter().map(|i| value(i)).collect::<Result<Vec<_>>>()?;
                let r = coll
                    .delete_many(doc! { "_id": { "$in": ids } })
                    .await
                    .map_err(err)?;
                ("delete", r.deleted_count)
            }
        };
        Ok(Outcome {
            command: Some(command.into()),
            rows_affected: Some(n),
        })
    }
}
