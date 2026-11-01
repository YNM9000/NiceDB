use nd_driver::{Chunk, Connection, DriverError};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
    Activity, Grant, IndexUsage, Principal, Principals, RoleAttribute, ServerSession, Sizes,
    TableSize, ToolFault,
};
use crate::{CoreError, Result};

const SIZED: usize = 500;

type Found<T> = std::result::Result<T, DriverError>;

pub(crate) async fn docs(conn: &dyn Connection, command: &Value) -> Found<Vec<Value>> {
    let text = command.to_string();
    let (tx, mut rx) = mpsc::channel(8);
    let gather = async {
        let mut out = Vec::new();
        while let Some(chunk) = rx.recv().await {
            if let Chunk::Documents(d) = chunk {
                out.extend(d.iter().filter_map(|s| serde_json::from_str(s).ok()));
            }
        }
        out
    };
    let (outcome, found) = tokio::join!(conn.execute(&text, tx), gather);
    outcome.map(|_| found)
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Object(o) => o
            .get("$numberLong")
            .or_else(|| o.get("$numberInt"))
            .or_else(|| o.get("$numberDouble"))
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok()),
        _ => None,
    }
}

fn shown_command(op: &Value) -> Option<String> {
    let mut cmd = op.get("command")?.as_object()?.clone();
    for key in [
        "lsid",
        "$clusterTime",
        "$db",
        "$readPreference",
        "txnNumber",
    ] {
        cmd.remove(key);
    }
    (!cmd.is_empty()).then(|| Value::Object(cmd).to_string())
}

pub(crate) fn operation(op: &Value) -> Option<ServerSession> {
    let id = op.get("opid").and_then(text)?;
    let user = op
        .get("effectiveUsers")
        .and_then(Value::as_array)
        .and_then(|u| u.first())
        .and_then(|u| u.get("user"))
        .and_then(text);
    let database = op
        .get("ns")
        .and_then(Value::as_str)
        .and_then(|ns| ns.split('.').next())
        .filter(|d| !d.is_empty())
        .map(str::to_owned);
    let active = op.get("active").and_then(Value::as_bool).unwrap_or(false);
    Some(ServerSession {
        id,
        user,
        database,
        client: op.get("client").and_then(text),
        application: op.get("appName").and_then(text),
        state: Some(if active { "active" } else { "idle" }.to_owned()),
        query: shown_command(op),
        duration_ms: op
            .get("microsecs_running")
            .and_then(number)
            .map(|us| us / 1000.0),
        wait: op
            .get("waitingForLock")
            .and_then(Value::as_bool)
            .filter(|w| *w)
            .map(|_| "lock".to_owned()),
        blocked_by: Vec::new(),
    })
}

pub(crate) async fn activity(conn: &dyn Connection) -> Found<Activity> {
    let found = docs(
        conn,
        &json!({
            "aggregate": 1,
            "pipeline": [
                { "$currentOp": { "allUsers": true, "idleConnections": false } },
                { "$match": { "client": { "$exists": true }, "command.comment": { "$not": { "$regex": "^nicedb:tools" } } } },
                { "$sort": { "microsecs_running": -1 } },
                { "$limit": 1000 }
            ],
            "comment": "nicedb:tools",
            "$db": "admin"
        }),
    )
    .await?;
    Ok(Activity {
        sessions: found.iter().filter_map(operation).collect(),
        locks: Vec::new(),
        partial: false,
        can_cancel: true,
    })
}

pub(crate) fn kill_command(id: &str) -> Result<String> {
    let (shard, op) = id
        .rsplit_once(':')
        .map_or((None, id), |(s, o)| (Some(s), o));
    let shard_ok = shard.is_none_or(|s| {
        !s.is_empty()
            && s.len() <= 64
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    });
    if op.is_empty() || op.len() > 20 || !op.bytes().all(|b| b.is_ascii_digit()) || !shard_ok {
        return Err(ToolFault::Empty.into());
    }
    let op = match shard {
        Some(_) => Value::String(id.to_owned()),
        None => Value::Number(
            op.parse::<u64>()
                .map_err(|_| CoreError::Tool(ToolFault::Empty))?
                .into(),
        ),
    };
    Ok(json!({ "killOp": 1, "op": op, "$db": "admin" }).to_string())
}

fn index_of(
    name: &str,
    size: Option<f64>,
    stats: &[Value],
    namespace: &str,
    table: &str,
) -> IndexUsage {
    let stat = stats
        .iter()
        .find(|s| s.get("name").and_then(Value::as_str) == Some(name));
    let scans = stat
        .and_then(|s| s.get("accesses"))
        .and_then(|a| a.get("ops"))
        .and_then(number);
    let unique = name == "_id_"
        || stat
            .and_then(|s| s.get("spec"))
            .and_then(|s| s.get("unique"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
    IndexUsage {
        namespace: namespace.to_owned(),
        table: table.to_owned(),
        name: name.to_owned(),
        bytes: size,
        scans,
        unique,
        unused: scans == Some(0.0) && !unique,
    }
}

pub(crate) fn collection_size(
    namespace: &str,
    name: &str,
    stats: &Value,
    usage: &[Value],
) -> (TableSize, Vec<IndexUsage>) {
    let storage = stats.get("storageStats").unwrap_or(&Value::Null);
    let get = |k: &str| storage.get(k).and_then(number);
    let (data, index) = (get("storageSize"), get("totalIndexSize"));
    let table = TableSize {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        rows: get("count"),
        data_bytes: data,
        index_bytes: index,
        total_bytes: match (data, index) {
            (Some(d), i) => Some(d + i.unwrap_or(0.0)),
            (None, i) => i,
        },
    };
    let indexes = storage
        .get("indexSizes")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(ix, bytes)| index_of(ix, number(bytes), usage, namespace, name))
                .collect()
        })
        .unwrap_or_default();
    (table, indexes)
}

pub(crate) async fn sizes(conn: &dyn Connection, database: &str) -> Found<Sizes> {
    let listed = docs(
        conn,
        &json!({ "listCollections": 1, "nameOnly": true, "filter": { "type": "collection" } }),
    )
    .await?;
    let mut names: Vec<String> = listed
        .iter()
        .filter_map(|c| c.get("name").and_then(Value::as_str))
        .filter(|n| !n.starts_with("system."))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.truncate(SIZED);
    let (mut tables, mut indexes) = (Vec::new(), Vec::new());
    for name in names {
        let stats = docs(
            conn,
            &json!({ "aggregate": name, "pipeline": [{ "$collStats": { "storageStats": {} } }] }),
        )
        .await
        .unwrap_or_default();
        let usage = docs(
            conn,
            &json!({ "aggregate": name, "pipeline": [{ "$indexStats": {} }] }),
        )
        .await
        .unwrap_or_default();
        let (t, ix) = collection_size(
            database,
            &name,
            stats.first().unwrap_or(&Value::Null),
            &usage,
        );
        tables.push(t);
        indexes.extend(ix);
    }
    tables.sort_by(|a, b| {
        b.total_bytes
            .unwrap_or(0.0)
            .total_cmp(&a.total_bytes.unwrap_or(0.0))
    });
    indexes.sort_by(|a, b| b.bytes.unwrap_or(0.0).total_cmp(&a.bytes.unwrap_or(0.0)));
    Ok(Sizes {
        tables,
        indexes,
        usage_known: true,
        partitions: Vec::new(),
        hypertables: Vec::new(),
    })
}

fn role_names(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some((
                r.get("role")?.as_str()?.to_owned(),
                r.get("db")?.as_str()?.to_owned(),
            ))
        })
        .collect()
}

const SUPER: &[&str] = &["root", "__system"];
const MAKES_ROLES: &[&str] = &["root", "userAdmin", "userAdminAnyDatabase", "__system"];
const MAKES_DBS: &[&str] = &[
    "root",
    "readWriteAnyDatabase",
    "dbAdminAnyDatabase",
    "dbOwner",
    "__system",
];

fn user_principal(u: &Value, grants: &mut Vec<Grant>) -> Option<Principal> {
    let name = u.get("user").and_then(Value::as_str)?;
    let db = u.get("db").and_then(Value::as_str)?;
    let held = role_names(u.get("roles"));
    let has = |list: &[&str]| held.iter().any(|(r, _)| list.contains(&r.as_str()));
    let mut attributes = vec![RoleAttribute::Login];
    for (a, on) in [
        (RoleAttribute::Superuser, has(SUPER)),
        (RoleAttribute::CreateDb, has(MAKES_DBS)),
        (RoleAttribute::CreateRole, has(MAKES_ROLES)),
    ] {
        if on {
            attributes.push(a);
        }
    }
    let who = format!("{name}@{db}");
    grants.extend(held.iter().map(|(role, on)| Grant {
        grantee: who.clone(),
        kind: "database".into(),
        schema: None,
        object: Some(on.clone()),
        privilege: role.clone(),
        grantable: false,
    }));
    Some(Principal {
        name: name.to_owned(),
        host: Some(db.to_owned()),
        attributes,
        member_of: held.iter().map(|(r, d)| format!("{r}@{d}")).collect(),
    })
}

fn resource(p: &Value) -> (&'static str, Option<String>, Option<String>) {
    let r = p.get("resource").unwrap_or(&Value::Null);
    let db = r.get("db").and_then(Value::as_str).unwrap_or("");
    let coll = r.get("collection").and_then(Value::as_str).unwrap_or("");
    let named = |s: &str| Some(s.to_owned()).filter(|s| !s.is_empty());
    if r.get("cluster").and_then(Value::as_bool) == Some(true) {
        ("global", None, None)
    } else if coll.is_empty() {
        ("database", None, named(db))
    } else {
        ("table", named(db), named(coll))
    }
}

fn role_principal(r: &Value, grants: &mut Vec<Grant>) -> Option<Principal> {
    let name = r.get("role").and_then(Value::as_str)?;
    let db = r.get("db").and_then(Value::as_str)?;
    if r.get("isBuiltin").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let who = format!("{name}@{db}");
    for p in r
        .get("privileges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (kind, schema, object) = resource(p);
        for action in p
            .get("actions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            grants.push(Grant {
                grantee: who.clone(),
                kind: kind.into(),
                schema: schema.clone(),
                object: object.clone(),
                privilege: action.to_owned(),
                grantable: false,
            });
        }
    }
    Some(Principal {
        name: name.to_owned(),
        host: Some(db.to_owned()),
        attributes: Vec::new(),
        member_of: role_names(r.get("inheritedRoles").or_else(|| r.get("roles")))
            .into_iter()
            .map(|(r, d)| format!("{r}@{d}"))
            .collect(),
    })
}

pub(crate) fn principals_of(users: &[Value], roles: &[Value]) -> (Vec<Principal>, Vec<Grant>) {
    let mut grants = Vec::new();
    let mut principals: Vec<Principal> = users
        .iter()
        .filter_map(|u| user_principal(u, &mut grants))
        .collect();
    principals.extend(roles.iter().filter_map(|r| role_principal(r, &mut grants)));
    principals.sort_by(|a, b| (&a.host, &a.name).cmp(&(&b.host, &b.name)));
    principals.dedup_by(|a, b| a.name == b.name && a.host == b.host);
    (principals, grants)
}

pub(crate) async fn principals(conn: &dyn Connection, database: &str) -> Found<Principals> {
    let all = docs(
        conn,
        &json!({ "usersInfo": { "forAllDBs": true }, "$db": "admin" }),
    )
    .await;
    let (users, partial) = match all {
        Ok(found) => (found, false),
        Err(_) => (docs(conn, &json!({ "usersInfo": 1 })).await?, true),
    };
    let users: Vec<Value> = users
        .iter()
        .flat_map(|d| {
            d.get("users")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    let mut roles = Vec::new();
    let mut seen = vec!["admin".to_owned()];
    if !database.is_empty() && database != "admin" {
        seen.push(database.to_owned());
    }
    for db in &seen {
        if let Ok(found) = docs(
            conn,
            &json!({ "rolesInfo": 1, "showPrivileges": true, "showBuiltinRoles": false, "$db": db }),
        )
        .await
        {
            roles.extend(
                found
                    .iter()
                    .flat_map(|d| d.get("roles").and_then(Value::as_array).cloned().unwrap_or_default()),
            );
        }
    }
    let (principals, grants) = principals_of(&users, &roles);
    Ok(Principals {
        principals,
        grants,
        partial,
        quotas: Vec::new(),
    })
}
