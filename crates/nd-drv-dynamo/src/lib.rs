mod item;
mod script;
mod sign;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use nd_driver::{
    Canceller, Capabilities, Chunk, ColumnInfo, CommandClass, Commands, Dependency, DocumentWrite,
    DriverError, DynamodbTarget, Family, Group, Index, Language, Namespace, NamespaceLevel, Noun,
    ObjectRef, Outcome, Relation, Report, ReportGroup, ReportSection, Result, ResultShape, Schema,
    SchemaKind, ServerInfo, Tool, TreeShape, Vocabulary,
};
use nd_http::Http;
use serde_json::{Map, Value, json};
use tokio::sync::{Notify, mpsc};

use crate::sign::Credentials;

const REPLY_CAP: usize = 64 * 1024 * 1024;
const SAMPLE: u64 = 50;
const MAX_SAMPLED_TABLES: usize = 200;

pub fn capabilities() -> Capabilities {
    Capabilities {
        family: Family::WideColumn,
        language: Language::Partiql,
        schema: SchemaKind::Inferred,
        results: ResultShape::Documents,
        tree: TreeShape {
            databases: false,
            namespaces: NamespaceLevel::Multiple,
            groups: vec![Group::Tables],
            server: false,
        },
        words: Vocabulary {
            table: Noun::Table,
            column: Noun::Field,
        },
        transactions: false,
        editable: true,
        explain: false,
        er: false,
        ddl: true,
        sort: false,
        params: false,
        tools: vec![Tool::Cluster],
        cancel: nd_driver::Cancel::BetweenFetches,
    }
}

fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let name = name.trim();
            inside =
                name == section || name.strip_prefix("profile ").map(str::trim) == Some(section);
            continue;
        }
        if inside
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().to_owned());
        }
    }
    None
}

fn aws_file(env: &str, name: &str) -> Option<String> {
    let path = std::env::var_os(env)
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".aws").join(name))
        })?;
    std::fs::read_to_string(path).ok()
}

fn credentials(t: &DynamodbTarget, secret: Option<&str>) -> Result<Credentials> {
    let profile = t.profile.trim();
    if !profile.is_empty() {
        let text = aws_file("AWS_SHARED_CREDENTIALS_FILE", "credentials").unwrap_or_default();
        let config = aws_file("AWS_CONFIG_FILE", "config").unwrap_or_default();
        let get = |k: &str| ini_value(&text, profile, k).or_else(|| ini_value(&config, profile, k));
        return match (get("aws_access_key_id"), get("aws_secret_access_key")) {
            (Some(key_id), Some(secret)) => Ok(Credentials {
                key_id,
                secret: zeroize::Zeroizing::new(secret),
                token: get("aws_session_token"),
            }),
            _ => Err(DriverError::Auth {
                message: format!("the AWS profile {profile} has no access key"),
            }),
        };
    }
    if !t.user.trim().is_empty() {
        return Ok(Credentials {
            key_id: t.user.trim().to_owned(),
            secret: zeroize::Zeroizing::new(secret.unwrap_or_default().to_owned()),
            token: None,
        });
    }
    match (
        std::env::var("AWS_ACCESS_KEY_ID"),
        std::env::var("AWS_SECRET_ACCESS_KEY"),
    ) {
        (Ok(key_id), Ok(secret)) => Ok(Credentials {
            key_id,
            secret: zeroize::Zeroizing::new(secret),
            token: std::env::var("AWS_SESSION_TOKEN").ok(),
        }),
        _ => Err(DriverError::Auth {
            message: "no credentials: give a profile or an access key".into(),
        }),
    }
}

struct Api {
    http: Http,
    region: String,
    creds: Credentials,
}

impl Api {
    async fn call(&self, op: &str, body: &Value) -> Result<Value> {
        let body = body.to_string().into_bytes();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let target = format!("DynamoDB_20120810.{op}");
        let signed = sign::headers(
            &self.creds,
            &self.region,
            &self.http.authority(),
            &target,
            &body,
            now,
        );
        let mut headers: Vec<(&str, &str)> = vec![("content-type", "application/x-amz-json-1.0")];
        headers.extend(signed.iter().map(|(k, v)| (*k, v.as_str())));
        let reply = self
            .http
            .send("POST", "/", &headers, Bytes::from(body))
            .await?;
        let status = reply.status;
        let text = reply.text(REPLY_CAP).await?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if status >= 400 {
            let kind = v
                .get("__type")
                .and_then(Value::as_str)
                .map(|t| t.rsplit('#').next().unwrap_or(t).to_owned());
            let message = v
                .get("message")
                .or_else(|| v.get("Message"))
                .and_then(Value::as_str)
                .map_or_else(|| format!("HTTP {status}: {}", text.trim()), str::to_owned);
            if matches!(
                kind.as_deref(),
                Some(
                    "UnrecognizedClientException"
                        | "InvalidSignatureException"
                        | "AccessDeniedException"
                        | "MissingAuthenticationTokenException"
                )
            ) {
                return Err(DriverError::Auth { message });
            }
            return Err(DriverError::Query {
                message,
                code: kind,
                detail: None,
                hint: None,
                offset: None,
            });
        }
        Ok(v)
    }
}

struct Cancel {
    stop: Notify,
}

struct DynamoCanceller(Arc<Cancel>);

#[async_trait::async_trait]
impl Canceller for DynamoCanceller {
    async fn cancel(&self) {
        self.0.stop.notify_waiters();
    }
}

pub struct DynamoConnection {
    api: Api,
    cancel: Arc<Cancel>,
    server: ServerInfo,
    keys: Mutex<HashMap<String, Vec<String>>>,
}

pub async fn connect(t: &DynamodbTarget, secret: Option<&str>) -> Result<DynamoConnection> {
    let region = t.region.trim();
    if region.is_empty() {
        return Err(DriverError::Connect {
            message: "a region is needed, such as eu-west-1".into(),
        });
    }
    let endpoint = if t.endpoint.trim().is_empty() {
        format!("https://dynamodb.{region}.amazonaws.com")
    } else {
        t.endpoint.trim().to_owned()
    };
    let (https, host, port) =
        nd_http::parse_url(&endpoint).ok_or_else(|| DriverError::Connect {
            message: format!("{endpoint} is not an http:// or https:// address"),
        })?;
    let tls = if https {
        nd_http::tls_config(nd_driver::TlsMode::Verify, None, None)?
    } else {
        None
    };
    let api = Api {
        http: Http::new(&host, port, None, tls),
        region: region.to_owned(),
        creds: credentials(t, secret)?,
    };
    api.call("ListTables", &json!({ "Limit": 1 }))
        .await
        .map_err(|e| match e {
            DriverError::Query { message, .. } => DriverError::Connect { message },
            other => other,
        })?;
    Ok(DynamoConnection {
        api,
        cancel: Arc::new(Cancel {
            stop: Notify::new(),
        }),
        server: ServerInfo {
            product: if t.endpoint.trim().is_empty() {
                "DynamoDB"
            } else {
                "DynamoDB Local"
            }
            .into(),
            version: "2012-08-10".into(),
            database: region.to_owned(),
        },
        keys: Mutex::new(HashMap::new()),
    })
}

fn key_names(table: &Value) -> Vec<String> {
    let mut keys: Vec<(bool, String)> = table
        .get("KeySchema")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|k| {
            Some((
                k.get("KeyType")?.as_str()? == "RANGE",
                k.get("AttributeName")?.as_str()?.to_owned(),
            ))
        })
        .collect();
    keys.sort_by_key(|(range, _)| *range);
    keys.into_iter().map(|(_, n)| n).collect()
}

fn statement_table(sql: &str) -> Option<String> {
    let words: Vec<&str> = sql.split_whitespace().collect();
    let at = words
        .iter()
        .position(|w| w.eq_ignore_ascii_case("FROM") || w.eq_ignore_ascii_case("INTO"))
        .map(|i| i + 1)
        .or_else(|| {
            words
                .first()
                .filter(|w| w.eq_ignore_ascii_case("UPDATE"))
                .map(|_| 1)
        })?;
    let name = words.get(at)?;
    let name = name.split('.').next().unwrap_or(name);
    Some(name.trim_matches('"').to_owned())
}

fn type_name(av: &Value) -> &str {
    av.as_object()
        .and_then(|o| o.keys().next())
        .map_or("?", String::as_str)
}

impl DynamoConnection {
    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let stop = self.cancel.stop.notified();
        tokio::select! {
            r = work => r,
            () = stop => Err(DriverError::Cancelled),
        }
    }

    async fn describe(&self, table: &str) -> Result<Value> {
        let v = self
            .api
            .call("DescribeTable", &json!({ "TableName": table }))
            .await?;
        Ok(v.get("Table").cloned().unwrap_or(Value::Null))
    }

    async fn keys_of(&self, table: &str) -> Result<Vec<String>> {
        if let Some(k) = self.keys.lock().ok().and_then(|m| m.get(table).cloned()) {
            return Ok(k);
        }
        let keys = key_names(&self.describe(table).await?);
        if let Ok(mut m) = self.keys.lock() {
            m.insert(table.to_owned(), keys.clone());
        }
        Ok(keys)
    }

    async fn emit(
        &self,
        items: &[Value],
        keys: &[String],
        out: &mpsc::Sender<Chunk>,
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let docs = items
            .iter()
            .filter_map(Value::as_object)
            .map(|i| item::item_to_document(i, keys))
            .collect();
        out.send(Chunk::Documents(docs))
            .await
            .map_err(|_| DriverError::Cancelled)
    }

    async fn statement(
        &self,
        sql: &str,
        write: bool,
        out: &mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let keys = match statement_table(sql) {
            Some(t) => self.keys_of(&t).await.unwrap_or_default(),
            None => Vec::new(),
        };
        let mut req = json!({ "Statement": sql });
        loop {
            let v = self.api.call("ExecuteStatement", &req).await?;
            let items = v
                .get("Items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            self.emit(&items, &keys, out).await?;
            match v.get("NextToken").and_then(Value::as_str) {
                Some(t) => req["NextToken"] = Value::from(t),
                None => break,
            }
        }
        Ok(Outcome {
            command: sql.split_whitespace().next().map(str::to_uppercase),
            rows_affected: write.then_some(1),
        })
    }

    async fn api_call(
        &self,
        op: &str,
        req: Map<String, Value>,
        out: &mpsc::Sender<Chunk>,
    ) -> Result<Outcome> {
        let table = req
            .get("TableName")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let keys = match &table {
            Some(t) if matches!(op, "Scan" | "Query" | "GetItem") => {
                self.keys_of(t).await.unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let mut req = Value::Object(req);
        let one_page = req.get("Limit").is_some();
        let outcome = |n| Outcome {
            command: Some(op.to_owned()),
            rows_affected: n,
        };
        match op {
            "Scan" | "Query" => {
                loop {
                    let v = self.api.call(op, &req).await?;
                    let items = v
                        .get("Items")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    self.emit(&items, &keys, out).await?;
                    match v.get("LastEvaluatedKey") {
                        Some(k) if !one_page => req["ExclusiveStartKey"] = k.clone(),
                        _ => break,
                    }
                }
                Ok(outcome(None))
            }
            "GetItem" => {
                let v = self.api.call(op, &req).await?;
                if let Some(i) = v.get("Item") {
                    self.emit(std::slice::from_ref(i), &keys, out).await?;
                }
                Ok(outcome(None))
            }
            "BatchGetItem" => {
                let v = self.api.call(op, &req).await?;
                for (t, items) in v
                    .get("Responses")
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                {
                    let keys = self.keys_of(t).await.unwrap_or_default();
                    self.emit(items.as_array().map_or(&[][..], Vec::as_slice), &keys, out)
                        .await?;
                }
                Ok(outcome(None))
            }
            _ => {
                let v = self.api.call(op, &req).await?;
                out.send(Chunk::Documents(vec![v.to_string()]))
                    .await
                    .map_err(|_| DriverError::Cancelled)?;
                let writes_item = matches!(op, "PutItem" | "UpdateItem" | "DeleteItem");
                Ok(outcome(writes_item.then_some(1)))
            }
        }
    }

    async fn tables(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let mut req = json!({});
        loop {
            let v = self.api.call("ListTables", &req).await?;
            out.extend(
                v.get("TableNames")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned),
            );
            match v.get("LastEvaluatedTableName") {
                Some(n) => req["ExclusiveStartTableName"] = n.clone(),
                None => break,
            }
        }
        Ok(out)
    }

    #[allow(clippy::too_many_lines)]
    async fn relation(&self, name: &str, sample: bool) -> Result<Relation> {
        let t = self.describe(name).await?;
        let keys = key_names(&t);
        if let Ok(mut m) = self.keys.lock() {
            m.insert(name.to_owned(), keys.clone());
        }
        let declared: HashMap<String, String> = t
            .get("AttributeDefinitions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|a| {
                Some((
                    a.get("AttributeName")?.as_str()?.to_owned(),
                    a.get("AttributeType")?.as_str()?.to_owned(),
                ))
            })
            .collect();
        let mut columns: Vec<ColumnInfo> = keys
            .iter()
            .map(|k| ColumnInfo {
                name: k.clone(),
                type_name: declared.get(k).cloned().unwrap_or_default(),
                nullable: false,
                primary_key: true,
            })
            .collect();
        let mut seen: Vec<(String, Vec<String>)> = Vec::new();
        if sample {
            let v = self
                .api
                .call("Scan", &json!({ "TableName": name, "Limit": SAMPLE }))
                .await?;
            for it in v
                .get("Items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                for (k, av) in it.as_object().into_iter().flatten() {
                    if keys.contains(k) {
                        continue;
                    }
                    let kind = type_name(av).to_owned();
                    match seen.iter_mut().find(|(n, _)| n == k) {
                        Some((_, kinds)) => kinds.push(kind),
                        None => seen.push((k.clone(), vec![kind])),
                    }
                }
            }
            seen.sort_by(|a, b| a.0.cmp(&b.0));
        }
        for (k, kinds) in seen {
            let mut distinct: Vec<&String> = Vec::new();
            for x in &kinds {
                if !distinct.contains(&x) {
                    distinct.push(x);
                }
            }
            let type_name = if distinct.len() == 1 {
                distinct[0].clone()
            } else {
                distinct
                    .iter()
                    .map(|d| {
                        let n = kinds.iter().filter(|x| x == d).count();
                        format!("{d} {}%", n * 100 / kinds.len())
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            columns.push(ColumnInfo {
                name: k,
                type_name,
                nullable: true,
                primary_key: false,
            });
        }
        for (k, ty) in &declared {
            if !columns.iter().any(|c| &c.name == k) {
                columns.push(ColumnInfo {
                    name: k.clone(),
                    type_name: ty.clone(),
                    nullable: true,
                    primary_key: false,
                });
            }
        }
        let mut indexes = Vec::new();
        for list in ["GlobalSecondaryIndexes", "LocalSecondaryIndexes"] {
            for ix in t.get(list).and_then(Value::as_array).into_iter().flatten() {
                let Some(n) = ix.get("IndexName").and_then(Value::as_str) else {
                    continue;
                };
                indexes.push(Index {
                    name: n.to_owned(),
                    columns: key_names(ix),
                    unique: false,
                });
            }
        }
        Ok(Relation {
            name: name.to_owned(),
            columns,
            indexes,
            foreign_keys: Vec::new(),
            estimated_rows: t.get("ItemCount").and_then(Value::as_f64),
            detail: Some(billing(&t)),
        })
    }

    async fn write_item(
        &self,
        table: &str,
        item: Map<String, Value>,
        condition: &str,
        keys: &[String],
    ) -> Result<()> {
        let names: Map<String, Value> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (format!("#k{i}"), Value::from(k.as_str())))
            .collect();
        let cond = (0..keys.len())
            .map(|i| format!("{condition}(#k{i})"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let req = json!({
            "TableName": table,
            "Item": item,
            "ConditionExpression": cond,
            "ExpressionAttributeNames": names,
        });
        self.api
            .call("PutItem", &req)
            .await
            .map(|_| ())
            .map_err(|e| gone_or(e, condition))
    }
}

fn gone_or(e: DriverError, condition: &str) -> DriverError {
    match &e {
        DriverError::Query { code: Some(c), .. } if c == "ConditionalCheckFailedException" => {
            DriverError::query(if condition == "attribute_exists" {
                "the item is no longer there"
            } else {
                "an item with this key already exists"
            })
        }
        _ => e,
    }
}

fn billing(t: &Value) -> String {
    let mode = t
        .get("BillingModeSummary")
        .and_then(|b| b.get("BillingMode"))
        .and_then(Value::as_str)
        .unwrap_or("PROVISIONED");
    if mode == "PAY_PER_REQUEST" {
        return "on demand".into();
    }
    let pt = t.get("ProvisionedThroughput");
    let n = |k: &str| {
        pt.and_then(|p| p.get(k))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    format!(
        "{} RCU · {} WCU",
        n("ReadCapacityUnits"),
        n("WriteCapacityUnits")
    )
}

fn create_call(t: &Value) -> Value {
    let pick = |v: &Value, keys: &[&str]| -> Value {
        Value::Object(
            keys.iter()
                .filter_map(|k| v.get(*k).map(|x| ((*k).to_owned(), x.clone())))
                .collect(),
        )
    };
    let on_demand = t
        .get("BillingModeSummary")
        .and_then(|b| b.get("BillingMode"))
        .and_then(Value::as_str)
        == Some("PAY_PER_REQUEST");
    let throughput = |v: &Value| {
        pick(
            v.get("ProvisionedThroughput").unwrap_or(&Value::Null),
            &["ReadCapacityUnits", "WriteCapacityUnits"],
        )
    };
    let mut req = pick(t, &["TableName", "KeySchema", "AttributeDefinitions"]);
    req["BillingMode"] = Value::from(if on_demand {
        "PAY_PER_REQUEST"
    } else {
        "PROVISIONED"
    });
    if !on_demand {
        req["ProvisionedThroughput"] = throughput(t);
    }
    for list in ["GlobalSecondaryIndexes", "LocalSecondaryIndexes"] {
        if let Some(ixs) = t
            .get(list)
            .and_then(Value::as_array)
            .filter(|l| !l.is_empty())
        {
            req[list] = Value::Array(
                ixs.iter()
                    .map(|ix| {
                        let mut out = pick(ix, &["IndexName", "KeySchema", "Projection"]);
                        if list.starts_with("Global") && !on_demand {
                            out["ProvisionedThroughput"] = throughput(ix);
                        }
                        out
                    })
                    .collect(),
            );
        }
    }
    json!({ "CreateTable": req })
}

#[async_trait::async_trait]
impl nd_driver::Connection for DynamoConnection {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }

    fn server(&self) -> &ServerInfo {
        &self.server
    }

    fn canceller(&self) -> Arc<dyn Canceller> {
        Arc::new(DynamoCanceller(self.cancel.clone()))
    }

    async fn execute(&self, text: &str, out: mpsc::Sender<Chunk>) -> Result<Outcome> {
        let text = text.trim();
        let write = script::classify(text).write;
        if text.starts_with('{') {
            let (op, req) = script::call(text).ok_or_else(|| {
                DriverError::query(
                    "write an API call as one JSON object: { \"Query\": { \"TableName\": … } }",
                )
            })?;
            return self.cancellable(self.api_call(&op, req, &out)).await;
        }
        self.cancellable(self.statement(text.trim_end_matches(';'), write, &out))
            .await
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
        let mut ns = Namespace::new(self.server.database.clone());
        let names = self.tables().await?;
        let sample = names.len() <= MAX_SAMPLED_TABLES;
        for name in names {
            match self.relation(&name, sample).await {
                Ok(r) => ns.tables.push(r),
                Err(e) => tracing::debug!(error = %e, "table skipped"),
            }
        }
        Ok(Schema {
            databases: vec![self.server.database.clone()],
            current_database: self.server.database.clone(),
            namespaces: vec![ns],
        })
    }

    async fn relation(&self, _namespace: &str, name: &str) -> Result<Option<Relation>> {
        Ok(Some(DynamoConnection::relation(self, name, true).await?))
    }

    async fn ddl(&self, object: &ObjectRef) -> Result<String> {
        let t = self.describe(&object.name).await?;
        if t.is_null() {
            return Err(object.not_found());
        }
        Ok(serde_json::to_string_pretty(&create_call(&t)).unwrap_or_default())
    }

    async fn dependencies(&self, _object: &ObjectRef) -> Result<Vec<Dependency>> {
        Ok(Vec::new())
    }

    fn commands(&self, text: &str) -> Commands {
        script::commands(text)
    }

    fn classify(&self, command: &str) -> CommandClass {
        script::classify(command)
    }

    async fn ping(&self) -> Result<()> {
        self.api
            .call("ListTables", &json!({ "Limit": 1 }))
            .await
            .map(|_| ())
    }

    async fn write_documents(&self, write: &DocumentWrite) -> Result<Outcome> {
        let (_, table) = write.collection();
        let keys = self.keys_of(table).await?;
        if keys.is_empty() {
            return Err(DriverError::query(format!("{table} has no key")));
        }
        let mut n = 0;
        match write {
            DocumentWrite::Insert { documents, .. } => {
                for d in documents {
                    let item = item::document_to_item(d).map_err(DriverError::query)?;
                    if let Some(k) = keys.iter().find(|k| !item.contains_key(*k)) {
                        return Err(DriverError::query(format!(
                            "the item needs its key attribute {k}"
                        )));
                    }
                    self.write_item(table, item, "attribute_not_exists", &keys)
                        .await?;
                    n += 1;
                }
            }
            DocumentWrite::Replace { id, document, .. } => {
                let mut item = item::document_to_item(document).map_err(DriverError::query)?;
                let key = item::document_to_item(id).map_err(DriverError::query)?;
                for (k, v) in key {
                    item.insert(k, v);
                }
                self.write_item(table, item, "attribute_exists", &keys)
                    .await?;
                n += 1;
            }
            DocumentWrite::Delete { ids, .. } => {
                for id in ids {
                    let key = item::document_to_item(id).map_err(DriverError::query)?;
                    let names: Map<String, Value> = keys
                        .iter()
                        .enumerate()
                        .map(|(i, k)| (format!("#k{i}"), Value::from(k.as_str())))
                        .collect();
                    let cond = (0..keys.len())
                        .map(|i| format!("attribute_exists(#k{i})"))
                        .collect::<Vec<_>>()
                        .join(" AND ");
                    let req = json!({ "TableName": table, "Key": key, "ConditionExpression": cond, "ExpressionAttributeNames": names });
                    self.api
                        .call("DeleteItem", &req)
                        .await
                        .map_err(|e| gone_or(e, "attribute_exists"))?;
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
        let (_, table) = write.collection();
        let item = |d: &str| {
            item::document_to_item(d).map_or_else(|_| Value::String(d.to_owned()), Value::Object)
        };
        let calls: Vec<Value> = match write {
            DocumentWrite::Insert { documents, .. } => documents
                .iter()
                .map(|d| json!({ "PutItem": { "TableName": table, "Item": item(d) } }))
                .collect(),
            DocumentWrite::Replace { id, document, .. } => {
                let mut full = item(document);
                if let (Value::Object(f), Value::Object(k)) = (&mut full, item(id)) {
                    f.extend(k);
                }
                vec![json!({ "PutItem": { "TableName": table, "Item": full } })]
            }
            DocumentWrite::Delete { ids, .. } => ids
                .iter()
                .map(|id| json!({ "DeleteItem": { "TableName": table, "Key": item(id) } }))
                .collect(),
        };
        calls
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn report(&self) -> Result<Report> {
        let names = self.tables().await?;
        let mut rows = Vec::new();
        let (mut items, mut bytes) = (0u64, 0u64);
        for name in &names {
            let Ok(t) = self.describe(name).await else {
                continue;
            };
            let n = |k: &str| t.get(k).and_then(Value::as_u64).unwrap_or(0);
            items += n("ItemCount");
            bytes += n("TableSizeBytes");
            let gsi = t
                .get("GlobalSecondaryIndexes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let lsi = t
                .get("LocalSecondaryIndexes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            rows.push(vec![
                name.clone(),
                t.get("TableStatus")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                n("ItemCount").to_string(),
                n("TableSizeBytes").to_string(),
                billing(&t),
                key_names(&t).join(", "),
                gsi.to_string(),
                lsi.to_string(),
            ]);
        }
        let info = vec![
            vec!["product".into(), self.server.product.clone()],
            vec!["region".into(), self.server.database.clone()],
            vec!["endpoint".into(), self.api.http.authority()],
            vec!["tables".into(), names.len().to_string()],
            vec!["items".into(), items.to_string()],
            vec!["size_bytes".into(), bytes.to_string()],
        ];
        Ok(Report {
            sections: vec![
                ReportSection {
                    id: "region".into(),
                    group: ReportGroup::Info,
                    columns: vec!["name".into(), "value".into()],
                    rows: info,
                },
                ReportSection {
                    id: "tables".into(),
                    group: ReportGroup::Grid,
                    columns: [
                        "table",
                        "status",
                        "items",
                        "size_bytes",
                        "capacity",
                        "key",
                        "gsi",
                        "lsi",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                    rows,
                },
            ],
        })
    }
}
