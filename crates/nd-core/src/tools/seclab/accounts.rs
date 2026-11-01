use std::collections::BTreeMap;

use super::{Ctx, Outcome, SecSeverity, SecSkip, cell, found, on, rows, subject};
use crate::tools::{Engine, Grant, Principal, RoleAttribute};

const DEFAULT_NAMES: &[&str] = &[
    "sa",
    "root",
    "postgres",
    "admin",
    "administrator",
    "mysql",
    "sys",
    "system",
];
const INTERNAL: &[&str] = &[
    "mysql.sys",
    "mysql.session",
    "mysql.infoschema",
    "mariadb.sys",
    "dbo",
];
const WRITING: &[&str] = &[
    "INSERT",
    "UPDATE",
    "DELETE",
    "TRUNCATE",
    "CREATE",
    "ALL",
    "ALL PRIVILEGES",
    "ALTER",
    "DROP",
    "CONTROL",
];
const PASSWORD_HINT: &str = "'{{password}}'";

pub(super) fn label(p: &Principal) -> String {
    match &p.host {
        Some(h) => format!("{}@{h}", p.name),
        None => p.name.clone(),
    }
}

fn principal_sql(cx: &Ctx<'_>, p: &Principal) -> String {
    match &p.host {
        Some(h) => format!(
            "{}@{}",
            nd_sql::literal(cx.dialect(), Some(&p.name)),
            nd_sql::literal(cx.dialect(), Some(h))
        ),
        None => cx.q(&p.name),
    }
}

fn active_logins<'a>(cx: &'a Ctx<'_>) -> impl Iterator<Item = &'a Principal> {
    cx.principals
        .into_iter()
        .flat_map(|p| p.principals.iter())
        .filter(|p| {
            p.attributes.contains(&RoleAttribute::Login)
                && !p.attributes.contains(&RoleAttribute::Locked)
                && !INTERNAL.contains(&p.name.as_str())
                && (cx.engine != Engine::Cockroach || p.name != "admin")
        })
}

fn is_super(p: &Principal) -> bool {
    p.attributes.contains(&RoleAttribute::Superuser)
}

pub(super) fn superusers(cx: &Ctx<'_>) -> Outcome {
    let Some(_) = cx.principals else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let list: Vec<&Principal> = active_logins(cx).filter(|p| is_super(p)).collect();
    if list.is_empty() {
        return Outcome::Passed;
    }
    let mut fix = Vec::new();
    for p in &list {
        let who = principal_sql(cx, p);
        match cx.engine {
            Engine::Postgres => fix.push(format!("ALTER ROLE {who} NOSUPERUSER;")),
            Engine::Cockroach => fix.push(format!("REVOKE admin FROM {who};")),
            Engine::Mssql => {
                fix.push(format!("ALTER SERVER ROLE sysadmin DROP MEMBER {who};"));
                fix.push(format!("ALTER ROLE db_owner DROP MEMBER {who};"));
            }
            Engine::Clickhouse => fix.push(format!("REVOKE ALL ON *.* FROM {who};")),
            _ => fix.push(format!("REVOKE ALL PRIVILEGES, GRANT OPTION FROM {who};")),
        }
    }
    let severity = if list.len() > 1 {
        SecSeverity::Medium
    } else {
        SecSeverity::Low
    };
    found(
        severity,
        list.iter().map(|p| subject(label(p), None)).collect(),
        fix,
    )
}

pub(super) async fn default_accounts(cx: &Ctx<'_>) -> Outcome {
    let Some(_) = cx.principals else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let mut subjects = Vec::new();
    let mut fix = Vec::new();
    let named = |p: &&Principal| {
        let name = p.name.to_lowercase();
        DEFAULT_NAMES.contains(&name.as_str())
            || (cx.engine == Engine::Clickhouse && name == "default")
    };
    for p in active_logins(cx).filter(named) {
        let local = matches!(p.host.as_deref(), Some("localhost" | "127.0.0.1" | "::1"));
        if matches!(cx.engine, Engine::Mysql | Engine::Mariadb) && local {
            continue;
        }
        let who = principal_sql(cx, p);
        subjects.push(subject(label(p), None));
        match cx.engine {
            Engine::Postgres | Engine::Cockroach => {
                fix.push(format!(
                    "-- create another administrator first, then:\nALTER ROLE {who} NOLOGIN;"
                ));
            }
            Engine::Clickhouse => fix.push(format!("ALTER USER {who} HOST LOCAL;")),
            _ => fix.push(format!("ALTER USER {who} ACCOUNT LOCK;")),
        }
    }
    if cx.engine == Engine::Mssql {
        let sa = rows(
            cx.conn,
            "SELECT name FROM sys.server_principals WHERE name = 'sa' AND is_disabled = 0",
        )
        .await;
        if sa.is_some_and(|r| !r.is_empty()) {
            subjects.push(subject("sa", None));
            fix.push("ALTER LOGIN [sa] DISABLE;".into());
        }
    }
    if subjects.is_empty() {
        return Outcome::Passed;
    }
    found(SecSeverity::Medium, subjects, fix)
}

pub(super) fn anonymous(cx: &Ctx<'_>) -> Outcome {
    let Some(pr) = cx.principals else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let list: Vec<&Principal> = pr.principals.iter().filter(|p| p.name.is_empty()).collect();
    if list.is_empty() {
        return Outcome::Passed;
    }
    let fix = list
        .iter()
        .map(|p| format!("DROP USER {};", principal_sql(cx, p)))
        .collect();
    found(
        SecSeverity::High,
        list.iter().map(|p| subject(label(p), None)).collect(),
        fix,
    )
}

#[allow(clippy::too_many_lines)]
pub(super) async fn weak_passwords(cx: &Ctx<'_>) -> Outcome {
    let (empty, weak, fix) = match cx.engine {
        Engine::Postgres => {
            let Some(r) = rows(
                cx.conn,
                "SELECT rolname, (rolpassword IS NULL)::text, (left(coalesce(rolpassword, ''), 3) = 'md5')::text \
FROM pg_authid WHERE rolcanlogin AND rolname NOT LIKE 'pg\\_%' ORDER BY rolname",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let mut fix = Vec::new();
            let (mut empty, mut weak) = (Vec::new(), Vec::new());
            for row in &r {
                let name = cell(row, 0);
                if on(row, 1) {
                    fix.push(format!(
                        "ALTER ROLE {} PASSWORD {PASSWORD_HINT};",
                        cx.q(&name)
                    ));
                    empty.push(subject(name, None));
                } else if on(row, 2) {
                    weak.push(subject(name, None));
                }
            }
            if !weak.is_empty() {
                fix.insert(0, "SET password_encryption = 'scram-sha-256';".into());
                fix.extend(
                    weak.iter()
                        .map(|s| format!("ALTER ROLE {} PASSWORD {PASSWORD_HINT};", cx.q(&s.name))),
                );
            }
            (empty, weak, fix)
        }
        Engine::Cockroach => return Outcome::Skip(SecSkip::NotDetectable),
        Engine::Clickhouse => {
            let Some(r) = rows(
                cx.conn,
                "SELECT name, toString(auth_type) FROM system.users ORDER BY name",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let (mut empty, mut weak, mut fix) = (Vec::new(), Vec::new(), Vec::new());
            for row in &r {
                let (name, auth) = (cell(row, 0), cell(row, 1));
                let plain = auth.contains("plaintext_password");
                if !auth.contains("no_password") && !plain {
                    continue;
                }
                fix.push(format!(
                    "ALTER USER {} IDENTIFIED WITH sha256_password BY {PASSWORD_HINT};",
                    cx.q(&name)
                ));
                if plain {
                    weak.push(subject(name, Some("plaintext".into())));
                } else {
                    empty.push(subject(name, None));
                }
            }
            (empty, weak, fix)
        }
        Engine::Mysql | Engine::Mariadb => {
            let Some(r) = rows(
                cx.conn,
                "SELECT User, Host, plugin, authentication_string FROM mysql.user ORDER BY User, Host",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let keyless = [
                "auth_socket",
                "unix_socket",
                "mysql_no_login",
                "auth_pam",
                "authentication_ldap_simple",
                "gssapi",
            ];
            let mut fix = Vec::new();
            let mut empty = Vec::new();
            for row in &r {
                let (user, host) = (cell(row, 0), cell(row, 1));
                let locked = cx.principals.is_some_and(|p| {
                    p.principals.iter().any(|x| {
                        x.name == user
                            && x.host.as_deref() == Some(&host)
                            && x.attributes.contains(&RoleAttribute::Locked)
                    })
                });
                if INTERNAL.contains(&user.as_str())
                    || locked
                    || !cell(row, 3).is_empty()
                    || keyless.contains(&cell(row, 2).as_str())
                {
                    continue;
                }
                fix.push(format!(
                    "ALTER USER {}@{} IDENTIFIED BY {PASSWORD_HINT};",
                    nd_sql::literal(cx.dialect(), Some(&user)),
                    nd_sql::literal(cx.dialect(), Some(&host))
                ));
                empty.push(subject(format!("{user}@{host}"), None));
            }
            (empty, Vec::new(), fix)
        }
        Engine::Mssql => {
            let Some(r) = rows(
                cx.conn,
                "SELECT name, CASE WHEN password_hash IS NULL THEN 1 ELSE 0 END, CASE WHEN PWDCOMPARE('', password_hash) = 1 THEN 1 ELSE 0 END, \
is_policy_checked FROM sys.sql_logins WHERE is_disabled = 0 ORDER BY name",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            if !r.is_empty() && r.iter().all(|row| on(row, 1)) {
                return Outcome::Skip(SecSkip::Privilege);
            }
            let (mut empty, mut weak, mut fix) = (Vec::new(), Vec::new(), Vec::new());
            for row in &r {
                let name = cell(row, 0);
                if on(row, 2) {
                    fix.push(format!(
                        "ALTER LOGIN {} WITH PASSWORD = {PASSWORD_HINT};",
                        cx.q(&name)
                    ));
                    empty.push(subject(name.clone(), None));
                }
                if !on(row, 3) {
                    fix.push(format!(
                        "ALTER LOGIN {} WITH CHECK_POLICY = ON;",
                        cx.q(&name)
                    ));
                    weak.push(subject(name, Some("policy".into())));
                }
            }
            (empty, weak, fix)
        }
        _ => return Outcome::Skip(SecSkip::Engine),
    };
    if empty.is_empty() && weak.is_empty() {
        return Outcome::Passed;
    }
    let severity = if empty.is_empty() {
        SecSeverity::Medium
    } else {
        SecSeverity::High
    };
    let mut subjects: Vec<_> = empty
        .into_iter()
        .map(|mut s| {
            s.detail = Some("empty".into());
            s
        })
        .collect();
    subjects.extend(weak.into_iter().map(|mut s| {
        s.detail.get_or_insert_with(|| "hash".into());
        s
    }));
    found(severity, subjects, fix)
}

fn schema_object(g: &Grant) -> String {
    [g.schema.as_deref(), g.object.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(".")
}

fn revoke(cx: &Ctx<'_>, privileges: &str, g: &Grant, who: &str) -> String {
    let q = |n: &str| cx.q(n);
    let name = |g: &Grant| match (&g.schema, &g.object) {
        (Some(s), Some(o)) => format!("{}.{}", q(s), q(o)),
        (None, Some(o)) => q(o),
        (Some(s), None) => q(s),
        (None, None) => String::new(),
    };
    match cx.engine {
        Engine::Mssql => match g.kind.as_str() {
            "database" => format!("REVOKE {privileges} FROM {who};"),
            "schema" => format!(
                "REVOKE {privileges} ON SCHEMA::{} FROM {who};",
                q(g.schema.as_deref().unwrap_or_default())
            ),
            _ => format!("REVOKE {privileges} ON {} FROM {who};", name(g)),
        },
        Engine::Mysql | Engine::Mariadb | Engine::Clickhouse => {
            let on = match g.kind.as_str() {
                "global" => "*.*".to_owned(),
                "schema" => format!("{}.*", q(g.schema.as_deref().unwrap_or_default())),
                "database" => format!("{}.*", q(g.object.as_deref().unwrap_or_default())),
                _ => name(g),
            };
            format!("REVOKE {privileges} ON {on} FROM {who};")
        }
        _ => {
            let target = match g.kind.as_str() {
                "database" => format!("DATABASE {}", q(g.object.as_deref().unwrap_or_default())),
                "schema" => format!("SCHEMA {}", q(g.schema.as_deref().unwrap_or_default())),
                "sequence" => format!("SEQUENCE {}", name(g)),
                _ => format!("TABLE {}", name(g)),
            };
            format!("REVOKE {privileges} ON {target} FROM {who};")
        }
    }
}

fn grouped<'a>(grants: impl Iterator<Item = &'a Grant>) -> Vec<(&'a Grant, Vec<String>)> {
    let mut map: BTreeMap<(String, String), (&Grant, Vec<String>)> = BTreeMap::new();
    for g in grants {
        let entry = map
            .entry((g.kind.clone(), schema_object(g)))
            .or_insert_with(|| (g, Vec::new()));
        if !entry.1.contains(&g.privilege) {
            entry.1.push(g.privilege.clone());
        }
    }
    map.into_values().collect()
}

pub(super) fn public_grants(cx: &Ctx<'_>) -> Outcome {
    let Some(pr) = cx.principals else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let wanted = if cx.engine == Engine::Mssql {
        "public"
    } else {
        "PUBLIC"
    };
    let open = pr.grants.iter().filter(|g| {
        g.grantee.eq_ignore_ascii_case(wanted)
            && g.privilege != "OWNER"
            && !(g.kind == "database"
                && matches!(g.privilege.as_str(), "CONNECT" | "TEMPORARY" | "TEMP"))
            && !(g.kind == "schema"
                && g.schema.as_deref() == Some("public")
                && g.privilege == "USAGE")
            && g.privilege != "CONNECT"
    });
    let groups = grouped(open);
    if groups.is_empty() {
        return Outcome::Passed;
    }
    let who = if cx.engine == Engine::Mssql {
        "[public]"
    } else {
        "PUBLIC"
    };
    let mut severity = SecSeverity::Medium;
    let mut subjects = Vec::new();
    let mut fix = Vec::new();
    for (g, privileges) in &groups {
        if privileges.iter().any(|p| WRITING.contains(&p.as_str())) {
            severity = SecSeverity::High;
        }
        let name = if g.kind == "database" {
            g.object.clone().unwrap_or_default()
        } else {
            schema_object(g)
        };
        subjects.push(subject(name, Some(privileges.join(", "))));
        fix.push(revoke(cx, &privileges.join(", "), g, who));
    }
    fix.truncate(80);
    found(severity, subjects, fix)
}

const DDL: &[&str] = &[
    "CREATE",
    "DROP",
    "ALTER",
    "CONTROL",
    "CREATE TABLE",
    "CREATE VIEW",
    "CREATE PROCEDURE",
    "CREATE FUNCTION",
    "CREATE SCHEMA",
    "INDEX",
];

pub(super) fn ddl_rights(cx: &Ctx<'_>) -> Outcome {
    let Some(pr) = cx.principals else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let mut subjects = Vec::new();
    let mut fix = Vec::new();
    for p in active_logins(cx).filter(|p| !is_super(p)) {
        let key = match &p.host {
            Some(h) => format!("{}@{h}", p.name),
            None => p.name.clone(),
        };
        let who = principal_sql(cx, p);
        let mut found: Vec<String> = Vec::new();
        for (g, privileges) in grouped(
            pr.grants
                .iter()
                .filter(|g| g.grantee == key && DDL.contains(&g.privilege.as_str())),
        ) {
            found.extend(privileges.iter().cloned());
            fix.push(revoke(cx, &privileges.join(", "), g, &who));
        }
        let attrs = [
            (RoleAttribute::CreateDb, "CREATEDB"),
            (RoleAttribute::CreateRole, "CREATEROLE"),
        ];
        if matches!(cx.engine, Engine::Postgres | Engine::Cockroach) {
            let held: Vec<&str> = attrs
                .iter()
                .filter(|(a, _)| p.attributes.contains(a))
                .map(|(_, n)| *n)
                .collect();
            if !held.is_empty() {
                fix.push(format!("ALTER ROLE {who} NOCREATEDB NOCREATEROLE;"));
                found.extend(held.iter().map(|s| (*s).to_owned()));
            }
        }
        for role in ["db_owner", "db_ddladmin"] {
            if cx.engine == Engine::Mssql && p.member_of.iter().any(|m| m == role) {
                fix.push(format!("ALTER ROLE {role} DROP MEMBER {who};"));
                found.push(role.into());
            }
        }
        if !found.is_empty() {
            found.dedup();
            subjects.push(subject(label(p), Some(found.join(", "))));
        }
    }
    if subjects.is_empty() {
        return Outcome::Passed;
    }
    fix.truncate(80);
    found(SecSeverity::Medium, subjects, fix)
}
