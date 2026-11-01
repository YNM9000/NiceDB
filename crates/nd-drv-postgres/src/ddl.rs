use std::fmt::Write as _;

use nd_driver::{DdlKind, ObjectRef, Result};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, Row};

use crate::Flavor;
use crate::error::map_error;

type Param<'a> = &'a (dyn ToSql + Sync);

async fn rows(client: &Client, sql: &str, params: &[Param<'_>]) -> Result<Vec<Row>> {
    client
        .query(sql, params)
        .await
        .map_err(|e| map_error(&e, sql))
}

async fn row(client: &Client, sql: &str, params: &[Param<'_>]) -> Result<Option<Row>> {
    client
        .query_opt(sql, params)
        .await
        .map_err(|e| map_error(&e, sql))
}

fn terminated(s: &str) -> String {
    let s = s.trim();
    if s.ends_with(';') {
        s.to_owned()
    } else {
        format!("{s};")
    }
}

pub(crate) async fn ddl(
    client: &Client,
    flavor: Flavor,
    version_num: i32,
    object: &ObjectRef,
) -> Result<String> {
    let ns = object.namespace.as_str();
    let name = object.name.as_str();
    match (object.kind, flavor) {
        (DdlKind::Table | DdlKind::View | DdlKind::Sequence, Flavor::Cockroach) => {
            show_create(client, ns, name).await
        }
        (DdlKind::Table | DdlKind::View, Flavor::Postgres) => {
            relation(client, ns, name, version_num).await
        }
        (DdlKind::Sequence, Flavor::Postgres) => sequence(client, ns, name).await,
        (DdlKind::Routine, _) => routine(client, object).await,
        (DdlKind::Trigger, _) => trigger(client, object).await,
        (DdlKind::Index, _) => index(client, ns, name).await,
    }
    .and_then(|found| found.ok_or_else(|| object.not_found()))
}

async fn show_create(client: &Client, ns: &str, name: &str) -> Result<Option<String>> {
    let sql = format!(
        "SHOW CREATE {}.{}",
        nd_driver::ident(nd_driver::Dialect::Postgres, ns),
        nd_driver::ident(nd_driver::Dialect::Postgres, name)
    );
    match client.query_opt(&sql, &[]).await {
        Ok(r) => Ok(r.map(|r| terminated(&r.get::<_, String>(1)))),
        Err(e) if e.as_db_error().is_some_and(|d| d.code().code() == "42P01") => Ok(None),
        Err(e) => Err(map_error(&e, &sql)),
    }
}

const HEAD: &str = "SELECT c.relkind::text, c.relpersistence::text,
        quote_ident(n.nspname) || '.' || quote_ident(c.relname),
        CASE WHEN c.relkind = 'p' THEN pg_get_partkeydef(c.oid) END,
        quote_literal(obj_description(c.oid, 'pg_class')),
        CASE WHEN c.relkind IN ('v', 'm') THEN pg_get_viewdef(c.oid, true) END,
        (SELECT quote_ident(s.srvname) FROM pg_foreign_table f JOIN pg_foreign_server s ON s.oid = f.ftserver WHERE f.ftrelid = c.oid)
     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind IN ('r', 'p', 'v', 'm', 'f')";

const COLUMNS: &str = "SELECT quote_ident(a.attname), format_type(a.atttypid, a.atttypmod), a.attnotnull,
        pg_get_expr(d.adbin, d.adrelid), a.attidentity::text, {generated},
        CASE WHEN a.attcollation <> 0 AND a.attcollation <> t.typcollation THEN quote_ident(co.collname) END,
        quote_literal(col_description(a.attrelid, a.attnum))
     FROM pg_attribute a
     JOIN pg_type t ON t.oid = a.atttypid
     LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
     LEFT JOIN pg_collation co ON co.oid = a.attcollation
     WHERE a.attrelid = $1::text::regclass AND a.attnum > 0 AND NOT a.attisdropped
     ORDER BY a.attnum";

const CONSTRAINTS: &str = "SELECT quote_ident(conname), pg_get_constraintdef(oid, true)
     FROM pg_constraint
     WHERE conrelid = $1::text::regclass AND contype IN ('p', 'u', 'c', 'x', 'f')
     ORDER BY CASE contype WHEN 'p' THEN 0 WHEN 'u' THEN 1 WHEN 'c' THEN 2 WHEN 'x' THEN 3 ELSE 4 END, conname";

const INDEXES: &str = "SELECT pg_get_indexdef(i.indexrelid)
     FROM pg_index i JOIN pg_class ic ON ic.oid = i.indexrelid
     WHERE i.indrelid = $1::text::regclass
       AND NOT EXISTS (SELECT 1 FROM pg_constraint k WHERE k.conindid = i.indexrelid AND k.contype IN ('p', 'u', 'x'))
     ORDER BY ic.relname";

const SECURITY: &str =
    "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE oid = $1::text::regclass";

const POLICIES: &str = "SELECT quote_ident(polname), polpermissive, polcmd::text,
        CASE WHEN polroles = '{0}' THEN 'PUBLIC'
             ELSE (SELECT string_agg(quote_ident(rolname), ', ' ORDER BY rolname) FROM pg_roles WHERE oid = ANY (polroles)) END,
        pg_get_expr(polqual, polrelid), pg_get_expr(polwithcheck, polrelid)
     FROM pg_policy WHERE polrelid = $1::text::regclass ORDER BY polname";

const TRIGGERS: &str = "SELECT pg_get_triggerdef(oid, true) FROM pg_trigger
     WHERE tgrelid = $1::text::regclass AND NOT tgisinternal ORDER BY tgname";

struct Head {
    kind: String,
    unlogged: bool,
    qualified: String,
    partition: Option<String>,
    comment: Option<String>,
    view: Option<String>,
    server: Option<String>,
}

async fn relation(
    client: &Client,
    ns: &str,
    name: &str,
    version_num: i32,
) -> Result<Option<String>> {
    let Some(r) = row(client, HEAD, &[&ns, &name]).await? else {
        return Ok(None);
    };
    let head = Head {
        kind: r.get(0),
        unlogged: r.get::<_, String>(1) == "u",
        qualified: r.get(2),
        partition: r.get(3),
        comment: r.get(4),
        view: r.get(5),
        server: r.get(6),
    };
    let q = head.qualified.as_str();
    let mut out = String::new();
    let mut comments = Vec::new();
    if let Some(c) = &head.comment {
        let what = if head.view.is_some() {
            if head.kind == "m" {
                "MATERIALIZED VIEW"
            } else {
                "VIEW"
            }
        } else if head.server.is_some() {
            "FOREIGN TABLE"
        } else {
            "TABLE"
        };
        comments.push(format!("COMMENT ON {what} {q} IS {c};"));
    }
    let generated = if version_num >= 120_000 {
        "a.attgenerated::text"
    } else {
        "''::text"
    };
    let columns = rows(client, &COLUMNS.replace("{generated}", generated), &[&q]).await?;
    for c in &columns {
        if let Some(text) = c.get::<_, Option<String>>(7) {
            let col: String = c.get(0);
            comments.push(format!("COMMENT ON COLUMN {q}.{col} IS {text};"));
        }
    }

    if let Some(def) = &head.view {
        let lead = if head.kind == "m" {
            "CREATE MATERIALIZED VIEW"
        } else {
            "CREATE OR REPLACE VIEW"
        };
        let _ = write!(out, "{lead} {q} AS\n{}", terminated(def.trim_start()));
    } else {
        let mut lines: Vec<String> = columns.iter().map(column_line).collect();
        for k in rows(client, CONSTRAINTS, &[&q]).await? {
            let (n, def): (String, String) = (k.get(0), k.get(1));
            lines.push(format!("    CONSTRAINT {n} {def}"));
        }
        let lead = match (&head.server, head.unlogged) {
            (Some(_), _) => "CREATE FOREIGN TABLE",
            (None, true) => "CREATE UNLOGGED TABLE",
            (None, false) => "CREATE TABLE",
        };
        let _ = write!(out, "{lead} {q} (\n{}\n)", lines.join(",\n"));
        if let Some(p) = &head.partition {
            let _ = write!(out, " PARTITION BY {p}");
        }
        if let Some(s) = &head.server {
            let _ = write!(out, " SERVER {s}");
        }
        out.push(';');
    }
    for i in rows(client, INDEXES, &[&q]).await? {
        let _ = write!(out, "\n\n{}", terminated(&i.get::<_, String>(0)));
    }
    for t in rows(client, TRIGGERS, &[&q]).await? {
        let _ = write!(out, "\n\n{}", terminated(&t.get::<_, String>(0)));
    }
    if head.view.is_none() {
        security(client, q, &mut out).await?;
    }
    if !comments.is_empty() {
        let _ = write!(out, "\n\n{}", comments.join("\n"));
    }
    Ok(Some(out))
}

async fn security(client: &Client, q: &str, out: &mut String) -> Result<()> {
    let Some(r) = row(client, SECURITY, &[&q]).await? else {
        return Ok(());
    };
    let (on, forced): (bool, bool) = (r.get(0), r.get(1));
    if on {
        let _ = write!(out, "\n\nALTER TABLE {q} ENABLE ROW LEVEL SECURITY;");
    }
    if forced {
        let _ = write!(out, "\nALTER TABLE {q} FORCE ROW LEVEL SECURITY;");
    }
    for p in rows(client, POLICIES, &[&q]).await? {
        let _ = write!(out, "\n\n{}", policy_line(q, &p));
    }
    Ok(())
}

fn policy_line(q: &str, p: &Row) -> String {
    let name: String = p.get(0);
    let permissive: bool = p.get(1);
    let command = match p.get::<_, String>(2).as_str() {
        "r" => "SELECT",
        "a" => "INSERT",
        "w" => "UPDATE",
        "d" => "DELETE",
        _ => "ALL",
    };
    let roles: Option<String> = p.get(3);
    let mut line = format!("CREATE POLICY {name} ON {q}");
    if !permissive {
        line.push_str(" AS RESTRICTIVE");
    }
    let _ = write!(line, "\n    FOR {command}");
    let _ = write!(line, "\n    TO {}", roles.as_deref().unwrap_or("PUBLIC"));
    if let Some(using) = p.get::<_, Option<String>>(4) {
        let _ = write!(line, "\n    USING ({using})");
    }
    if let Some(check) = p.get::<_, Option<String>>(5) {
        let _ = write!(line, "\n    WITH CHECK ({check})");
    }
    line.push(';');
    line
}

fn column_line(c: &Row) -> String {
    let name: String = c.get(0);
    let ty: String = c.get(1);
    let not_null: bool = c.get(2);
    let default: Option<String> = c.get(3);
    let identity: String = c.get(4);
    let generated: String = c.get(5);
    let collation: Option<String> = c.get(6);
    let mut line = format!("    {name} {ty}");
    if let Some(co) = collation {
        let _ = write!(line, " COLLATE {co}");
    }
    match (generated.as_str(), default) {
        ("s", Some(expr)) => {
            let _ = write!(line, " GENERATED ALWAYS AS ({expr}) STORED");
        }
        ("v", Some(expr)) => {
            let _ = write!(line, " GENERATED ALWAYS AS ({expr}) VIRTUAL");
        }
        (_, Some(expr)) => {
            let _ = write!(line, " DEFAULT {expr}");
        }
        _ => {}
    }
    match identity.as_str() {
        "a" => line.push_str(" GENERATED ALWAYS AS IDENTITY"),
        "d" => line.push_str(" GENERATED BY DEFAULT AS IDENTITY"),
        _ => {}
    }
    if not_null {
        line.push_str(" NOT NULL");
    }
    line
}

async fn sequence(client: &Client, ns: &str, name: &str) -> Result<Option<String>> {
    let Some(r) = row(
        client,
        "SELECT quote_ident(schemaname) || '.' || quote_ident(sequencename), data_type::text,
                start_value, min_value, max_value, increment_by, cycle, cache_size,
                (SELECT quote_ident(tn.nspname) || '.' || quote_ident(t.relname) || '.' || quote_ident(a.attname)
                 FROM pg_depend d
                 JOIN pg_class t ON t.oid = d.refobjid JOIN pg_namespace tn ON tn.oid = t.relnamespace
                 JOIN pg_attribute a ON a.attrelid = d.refobjid AND a.attnum = d.refobjsubid
                 WHERE d.classid = 'pg_class'::regclass AND d.refclassid = 'pg_class'::regclass
                   AND d.objid = (quote_ident(schemaname) || '.' || quote_ident(sequencename))::regclass
                   AND d.deptype = 'a')
         FROM pg_sequences WHERE schemaname = $1 AND sequencename = $2",
        &[&ns, &name],
    )
    .await?
    else {
        return Ok(None);
    };
    let q: String = r.get(0);
    let ty: String = r.get(1);
    let (start, min, max, inc): (i64, i64, i64, i64) = (r.get(2), r.get(3), r.get(4), r.get(5));
    let cycle: bool = r.get(6);
    let cache: i64 = r.get(7);
    let mut out = format!(
        "CREATE SEQUENCE {q}\n    AS {ty}\n    INCREMENT BY {inc}\n    MINVALUE {min}\n    MAXVALUE {max}\n    START WITH {start}\n    CACHE {cache}"
    );
    if cycle {
        out.push_str("\n    CYCLE");
    }
    out.push(';');
    if let Some(owner) = r.get::<_, Option<String>>(8) {
        let _ = write!(out, "\n\nALTER SEQUENCE {q} OWNED BY {owner};");
    }
    Ok(Some(out))
}

async fn routine(client: &Client, object: &ObjectRef) -> Result<Option<String>> {
    let sig = object.signature.as_deref();
    let Some(r) = row(
        client,
        "SELECT pg_get_functiondef(p.oid), p.prokind::text, quote_literal(obj_description(p.oid, 'pg_proc')),
                quote_ident(n.nspname) || '.' || quote_ident(p.proname) || '(' || pg_get_function_identity_arguments(p.oid) || ')'
         FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
         WHERE n.nspname = $1 AND p.proname = $2 AND p.prokind IN ('f', 'p')
           AND ($3::text IS NULL OR pg_get_function_identity_arguments(p.oid) = $3)
         ORDER BY p.oid LIMIT 1",
        &[&object.namespace, &object.name, &sig],
    )
    .await?
    else {
        return Ok(None);
    };
    let mut out = terminated(&r.get::<_, String>(0));
    if let Some(c) = r.get::<_, Option<String>>(2) {
        let what = if r.get::<_, String>(1) == "p" {
            "PROCEDURE"
        } else {
            "FUNCTION"
        };
        let _ = write!(
            out,
            "\n\nCOMMENT ON {what} {} IS {c};",
            r.get::<_, String>(3)
        );
    }
    Ok(Some(out))
}

async fn trigger(client: &Client, object: &ObjectRef) -> Result<Option<String>> {
    let table = object.table.as_deref();
    let Some(r) = row(
        client,
        "SELECT pg_get_triggerdef(g.oid, true), quote_literal(obj_description(g.oid, 'pg_trigger')),
                quote_ident(g.tgname), quote_ident(n.nspname) || '.' || quote_ident(c.relname)
         FROM pg_trigger g JOIN pg_class c ON c.oid = g.tgrelid JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = $1 AND g.tgname = $2 AND NOT g.tgisinternal
           AND ($3::text IS NULL OR c.relname = $3)
         ORDER BY c.relname LIMIT 1",
        &[&object.namespace, &object.name, &table],
    )
    .await?
    else {
        return Ok(None);
    };
    let mut out = terminated(&r.get::<_, String>(0));
    if let Some(c) = r.get::<_, Option<String>>(1) {
        let (t, on): (String, String) = (r.get(2), r.get(3));
        let _ = write!(out, "\n\nCOMMENT ON TRIGGER {t} ON {on} IS {c};");
    }
    Ok(Some(out))
}

async fn index(client: &Client, ns: &str, name: &str) -> Result<Option<String>> {
    let Some(r) = row(
        client,
        "SELECT pg_get_indexdef(i.indexrelid),
                (SELECT 'CONSTRAINT ' || quote_ident(k.conname) || ' ' || pg_get_constraintdef(k.oid, true)
                 FROM pg_constraint k WHERE k.conindid = i.indexrelid AND k.contype IN ('p', 'u', 'x') LIMIT 1),
                quote_ident(tn.nspname) || '.' || quote_ident(t.relname)
         FROM pg_index i
         JOIN pg_class ic ON ic.oid = i.indexrelid JOIN pg_namespace n ON n.oid = ic.relnamespace
         JOIN pg_class t ON t.oid = i.indrelid JOIN pg_namespace tn ON tn.oid = t.relnamespace
         WHERE n.nspname = $1 AND ic.relname = $2",
        &[&ns, &name],
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(match r.get::<_, Option<String>>(1) {
        Some(constraint) => format!("ALTER TABLE {} ADD {constraint};", r.get::<_, String>(2)),
        None => terminated(&r.get::<_, String>(0)),
    }))
}
