use std::fmt::Write as _;

use futures_util::TryStreamExt;
use nd_driver::{DdlKind, Dialect, DriverError, ObjectRef, Result, Row, ident};
use tiberius::{Query, QueryItem};

use crate::Tds;
use crate::error::map_error;
use crate::value;

pub(crate) async fn rows(client: &mut Tds, sql: &str, params: &[&str]) -> Result<Vec<Row>> {
    let run = async {
        let mut q = Query::new(sql);
        for p in params {
            q.bind(*p);
        }
        let mut stream = q.query(client).await?;
        let mut out = Vec::new();
        while let Some(item) = stream.try_next().await? {
            if let QueryItem::Row(row) = item {
                out.push(row.into_iter().map(value::text).collect());
            }
        }
        Ok::<_, tiberius::error::Error>(out)
    };
    run.await.map_err(|e| map_error(&e, sql))
}

fn cell(row: &Row, i: usize) -> Option<&str> {
    row.get(i).and_then(Option::as_deref)
}

fn text(row: &Row, i: usize) -> String {
    cell(row, i).unwrap_or_default().to_owned()
}

fn flag(row: &Row, i: usize) -> bool {
    cell(row, i) == Some("1")
}

fn q(name: &str) -> String {
    ident(Dialect::Mssql, name)
}

fn literal(s: &str) -> String {
    format!("N'{}'", s.replace('\'', "''"))
}

const BATCH: &str = "\nGO\n\n";

pub(crate) async fn ddl(client: &mut Tds, object: &ObjectRef) -> Result<String> {
    let ns = object.namespace.as_str();
    let name = object.name.as_str();
    match object.kind {
        DdlKind::Table => table(client, ns, name).await,
        DdlKind::View | DdlKind::Routine | DdlKind::Trigger => module(client, object).await,
        DdlKind::Index => index(client, object).await,
        DdlKind::Sequence => sequence(client, ns, name).await,
    }
}

async fn object_id(client: &mut Tds, ns: &str, name: &str, types: &str) -> Result<Option<String>> {
    let sql = format!(
        "SELECT o.object_id FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id
         WHERE s.name = @P1 AND o.name = @P2 AND o.type IN ({types})"
    );
    Ok(rows(client, &sql, &[ns, name])
        .await?
        .first()
        .map(|r| text(r, 0)))
}

async fn module(client: &mut Tds, object: &ObjectRef) -> Result<String> {
    let types = match object.kind {
        DdlKind::View => "'V'",
        DdlKind::Trigger => "'TR'",
        _ => "'P', 'FN', 'IF', 'TF', 'PC', 'FS', 'FT'",
    };
    let id = object_id(client, &object.namespace, &object.name, types)
        .await?
        .ok_or_else(|| object.not_found())?;
    let def = rows(client, "SELECT OBJECT_DEFINITION(CAST(@P1 AS int))", &[&id])
        .await?
        .first()
        .and_then(|r| cell(r, 0).map(str::to_owned))
        .ok_or_else(|| {
            DriverError::query(format!(
                "{}: definition not readable (encrypted, CLR or no VIEW DEFINITION)",
                object.name
            ))
        })?;
    let mut out = def.trim().to_owned();
    if object.kind == DdlKind::View {
        for i in indexes(client, &id, &qualified(&object.namespace, &object.name)).await? {
            let _ = write!(out, "{BATCH}{i}");
        }
    }
    Ok(out)
}

fn qualified(ns: &str, name: &str) -> String {
    format!("{}.{}", q(ns), q(name))
}

const COLUMNS: &str = "SELECT c.name, t.name, c.max_length, c.precision, c.scale, c.is_nullable, c.is_identity,
        CONVERT(nvarchar(40), ic.seed_value), CONVERT(nvarchar(40), ic.increment_value),
        cc.definition, cc.is_persisted, dc.name, dc.definition,
        CASE WHEN c.collation_name <> CONVERT(sysname, DATABASEPROPERTYEX(DB_NAME(), 'Collation')) THEN c.collation_name END,
        CASE WHEN t.is_user_defined = 1 THEN SCHEMA_NAME(t.schema_id) END,
        CONVERT(nvarchar(max), ep.value)
     FROM sys.columns c
     JOIN sys.types t ON t.user_type_id = c.user_type_id
     LEFT JOIN sys.identity_columns ic ON ic.object_id = c.object_id AND ic.column_id = c.column_id
     LEFT JOIN sys.computed_columns cc ON cc.object_id = c.object_id AND cc.column_id = c.column_id
     LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id
     LEFT JOIN sys.extended_properties ep ON ep.class = 1 AND ep.major_id = c.object_id AND ep.minor_id = c.column_id AND ep.name = 'MS_Description'
     WHERE c.object_id = CAST(@P1 AS int) ORDER BY c.column_id";

const KEY_COLUMNS: &str = "STRING_AGG(CONVERT(nvarchar(max), QUOTENAME(col.name) + CASE WHEN ixc.is_descending_key = 1 THEN ' DESC' ELSE ' ASC' END), ', ')
            WITHIN GROUP (ORDER BY ixc.key_ordinal)";

async fn table(client: &mut Tds, ns: &str, name: &str) -> Result<String> {
    let id = object_id(client, ns, name, "'U'")
        .await?
        .ok_or_else(|| DriverError::query(format!("{name} not found")))?;
    let table = qualified(ns, name);
    let columns = rows(client, COLUMNS, &[&id]).await?;
    let mut lines: Vec<String> = columns.iter().map(column_line).collect();
    let mut notes = Vec::new();
    for c in &columns {
        if let Some(d) = cell(c, 15) {
            notes.push(describe(ns, name, Some(&text(c, 0)), d));
        }
    }

    lines.extend(constraints(client, &id).await?);

    let mut out = format!("CREATE TABLE {table} (\n{}\n);", lines.join(",\n"));
    for i in indexes(client, &id, &table).await? {
        let _ = write!(out, "\n\n{i}");
    }
    let table_note = rows(
        client,
        "SELECT CONVERT(nvarchar(max), value) FROM sys.extended_properties WHERE class = 1 AND major_id = CAST(@P1 AS int) AND minor_id = 0 AND name = 'MS_Description'",
        &[&id],
    )
    .await?;
    if let Some(d) = table_note.first().and_then(|r| cell(r, 0)) {
        notes.insert(0, describe(ns, name, None, d));
    }
    if !notes.is_empty() {
        let _ = write!(out, "\n\n{}", notes.join("\n"));
    }
    for t in rows(
        client,
        "SELECT OBJECT_DEFINITION(object_id) FROM sys.triggers WHERE parent_id = CAST(@P1 AS int) ORDER BY name",
        &[&id],
    )
    .await?
    {
        if let Some(def) = cell(&t, 0) {
            let _ = write!(out, "{BATCH}{}", def.trim());
        }
    }
    Ok(out)
}

async fn constraints(client: &mut Tds, id: &str) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for k in rows(
        client,
        &format!(
            "SELECT kc.name, kc.type, i.type_desc,
                    (SELECT {KEY_COLUMNS} FROM sys.index_columns ixc
                     JOIN sys.columns col ON col.object_id = ixc.object_id AND col.column_id = ixc.column_id
                     WHERE ixc.object_id = i.object_id AND ixc.index_id = i.index_id AND ixc.key_ordinal > 0)
             FROM sys.key_constraints kc
             JOIN sys.indexes i ON i.object_id = kc.parent_object_id AND i.index_id = kc.unique_index_id
             WHERE kc.parent_object_id = CAST(@P1 AS int) ORDER BY kc.type, kc.name"
        ),
        &[id],
    )
    .await?
    {
        let what = if text(&k, 1).trim() == "PK" {
            "PRIMARY KEY"
        } else {
            "UNIQUE"
        };
        lines.push(format!(
            "    CONSTRAINT {} {what} {} ({})",
            q(&text(&k, 0)),
            text(&k, 2),
            text(&k, 3)
        ));
    }
    for k in rows(
        client,
        "SELECT name, definition FROM sys.check_constraints WHERE parent_object_id = CAST(@P1 AS int) ORDER BY name",
        &[id],
    )
    .await?
    {
        lines.push(format!(
            "    CONSTRAINT {} CHECK {}",
            q(&text(&k, 0)),
            text(&k, 1)
        ));
    }
    for k in rows(
        client,
        "SELECT fk.name, SCHEMA_NAME(ro.schema_id), ro.name,
                STRING_AGG(CONVERT(nvarchar(max), QUOTENAME(pc.name)), ', ') WITHIN GROUP (ORDER BY fkc.constraint_column_id),
                STRING_AGG(CONVERT(nvarchar(max), QUOTENAME(rc.name)), ', ') WITHIN GROUP (ORDER BY fkc.constraint_column_id),
                fk.delete_referential_action_desc, fk.update_referential_action_desc
         FROM sys.foreign_keys fk
         JOIN sys.objects ro ON ro.object_id = fk.referenced_object_id
         JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id
         JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id
         JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id
         WHERE fk.parent_object_id = CAST(@P1 AS int)
         GROUP BY fk.name, ro.schema_id, ro.name, fk.delete_referential_action_desc, fk.update_referential_action_desc
         ORDER BY fk.name",
        &[id],
    )
    .await?
    {
        let mut line = format!(
            "    CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
            q(&text(&k, 0)),
            text(&k, 3),
            qualified(&text(&k, 1), &text(&k, 2)),
            text(&k, 4)
        );
        for (rule, at) in [("DELETE", 5), ("UPDATE", 6)] {
            let action = text(&k, at);
            if action != "NO_ACTION" {
                let _ = write!(line, " ON {rule} {}", action.replace('_', " "));
            }
        }
        lines.push(line);
    }
    Ok(lines)
}

fn describe(ns: &str, table: &str, column: Option<&str>, text: &str) -> String {
    let mut s = format!(
        "EXEC sys.sp_addextendedproperty @name = N'MS_Description', @value = {}, @level0type = N'SCHEMA', @level0name = {}, @level1type = N'TABLE', @level1name = {}",
        literal(text),
        literal(ns),
        literal(table)
    );
    if let Some(c) = column {
        let _ = write!(s, ", @level2type = N'COLUMN', @level2name = {}", literal(c));
    }
    s.push(';');
    s
}

fn column_line(c: &Row) -> String {
    let name = q(&text(c, 0));
    if let Some(expr) = cell(c, 9) {
        let persisted = if flag(c, 10) { " PERSISTED" } else { "" };
        return format!("    {name} AS {expr}{persisted}");
    }
    let mut line = format!("    {name} {}", type_of(c));
    if let Some(co) = cell(c, 13) {
        let _ = write!(line, " COLLATE {co}");
    }
    if flag(c, 6) {
        let _ = write!(
            line,
            " IDENTITY({}, {})",
            cell(c, 7).unwrap_or("1"),
            cell(c, 8).unwrap_or("1")
        );
    }
    line.push_str(if flag(c, 5) { " NULL" } else { " NOT NULL" });
    if let (Some(df), Some(expr)) = (cell(c, 11), cell(c, 12)) {
        let _ = write!(line, " CONSTRAINT {} DEFAULT {expr}", q(df));
    }
    line
}

fn type_of(c: &Row) -> String {
    let ty = text(c, 1);
    if let Some(schema) = cell(c, 14) {
        return qualified(schema, &ty);
    }
    let len: i32 = cell(c, 2).and_then(|v| v.parse().ok()).unwrap_or(0);
    let precision = text(c, 3);
    let scale = text(c, 4);
    let sized = |n: i32| {
        if len == -1 {
            "max".to_owned()
        } else {
            n.to_string()
        }
    };
    match ty.as_str() {
        "varchar" | "char" | "varbinary" | "binary" => format!("{ty}({})", sized(len)),
        "nvarchar" | "nchar" => format!("{ty}({})", sized(len / 2)),
        "decimal" | "numeric" => format!("{ty}({precision}, {scale})"),
        "datetime2" | "time" | "datetimeoffset" => format!("{ty}({scale})"),
        _ => ty,
    }
}

async fn indexes(client: &mut Tds, id: &str, table: &str) -> Result<Vec<String>> {
    let found = rows(
        client,
        &format!(
            "SELECT i.name, i.is_unique, i.type_desc, i.filter_definition,
                    (SELECT {KEY_COLUMNS} FROM sys.index_columns ixc
                     JOIN sys.columns col ON col.object_id = ixc.object_id AND col.column_id = ixc.column_id
                     WHERE ixc.object_id = i.object_id AND ixc.index_id = i.index_id AND ixc.key_ordinal > 0),
                    (SELECT STRING_AGG(CONVERT(nvarchar(max), QUOTENAME(col.name)), ', ') WITHIN GROUP (ORDER BY ixc.index_column_id)
                     FROM sys.index_columns ixc
                     JOIN sys.columns col ON col.object_id = ixc.object_id AND col.column_id = ixc.column_id
                     WHERE ixc.object_id = i.object_id AND ixc.index_id = i.index_id AND (ixc.is_included_column = 1 OR i.type IN (5, 6)))
             FROM sys.indexes i
             WHERE i.object_id = CAST(@P1 AS int) AND i.type IN (1, 2, 5, 6)
               AND i.is_primary_key = 0 AND i.is_unique_constraint = 0 AND i.is_hypothetical = 0
             ORDER BY i.index_id"
        ),
        &[id],
    )
    .await?;
    Ok(found.iter().map(|r| index_sql(r, table)).collect())
}

fn index_sql(r: &Row, table: &str) -> String {
    let unique = if flag(r, 1) { "UNIQUE " } else { "" };
    let kind = text(r, 2);
    let name = q(&text(r, 0));
    if kind.ends_with("COLUMNSTORE") {
        let cols = cell(r, 5)
            .filter(|_| kind.starts_with("NONCLUSTERED"))
            .map(|c| format!(" ({c})"))
            .unwrap_or_default();
        return format!("CREATE {kind} INDEX {name} ON {table}{cols};");
    }
    let mut s = format!(
        "CREATE {unique}{kind} INDEX {name} ON {table} ({})",
        text(r, 4)
    );
    if let Some(inc) = cell(r, 5) {
        let _ = write!(s, " INCLUDE ({inc})");
    }
    if let Some(f) = cell(r, 3) {
        let _ = write!(s, " WHERE {f}");
    }
    s.push(';');
    s
}

async fn index(client: &mut Tds, object: &ObjectRef) -> Result<String> {
    let ns = object.namespace.as_str();
    let found = rows(
        client,
        &format!(
            "SELECT i.name, i.is_unique, i.type_desc, i.filter_definition,
                    (SELECT {KEY_COLUMNS} FROM sys.index_columns ixc
                     JOIN sys.columns col ON col.object_id = ixc.object_id AND col.column_id = ixc.column_id
                     WHERE ixc.object_id = i.object_id AND ixc.index_id = i.index_id AND ixc.key_ordinal > 0),
                    (SELECT STRING_AGG(CONVERT(nvarchar(max), QUOTENAME(col.name)), ', ') WITHIN GROUP (ORDER BY ixc.index_column_id)
                     FROM sys.index_columns ixc
                     JOIN sys.columns col ON col.object_id = ixc.object_id AND col.column_id = ixc.column_id
                     WHERE ixc.object_id = i.object_id AND ixc.index_id = i.index_id AND (ixc.is_included_column = 1 OR i.type IN (5, 6))),
                    o.name, i.is_primary_key, i.is_unique_constraint
             FROM sys.indexes i
             JOIN sys.objects o ON o.object_id = i.object_id
             WHERE SCHEMA_NAME(o.schema_id) = @P1 AND i.name = @P2 AND (@P3 = N'' OR o.name = @P3)
             ORDER BY o.name"
        ),
        &[ns, &object.name, object.table.as_deref().unwrap_or("")],
    )
    .await?;
    let r = found.first().ok_or_else(|| object.not_found())?;
    let table = qualified(ns, &text(r, 6));
    if flag(r, 7) || flag(r, 8) {
        let what = if flag(r, 7) { "PRIMARY KEY" } else { "UNIQUE" };
        return Ok(format!(
            "ALTER TABLE {table} ADD CONSTRAINT {} {what} {} ({});",
            q(&text(r, 0)),
            text(r, 2),
            text(r, 4)
        ));
    }
    Ok(index_sql(r, &table))
}

async fn sequence(client: &mut Tds, ns: &str, name: &str) -> Result<String> {
    let found = rows(
        client,
        "SELECT TYPE_NAME(q.user_type_id), CONVERT(nvarchar(40), q.start_value), CONVERT(nvarchar(40), q.increment),
                CONVERT(nvarchar(40), q.minimum_value), CONVERT(nvarchar(40), q.maximum_value),
                q.is_cycling, q.is_cached, q.cache_size
         FROM sys.sequences q WHERE SCHEMA_NAME(q.schema_id) = @P1 AND q.name = @P2",
        &[ns, name],
    )
    .await?;
    let r = found
        .first()
        .ok_or_else(|| DriverError::query(format!("{name} not found")))?;
    let cycle = if flag(r, 5) { "CYCLE" } else { "NO CYCLE" };
    let cache = match (flag(r, 6), cell(r, 7)) {
        (false, _) => "NO CACHE".to_owned(),
        (true, Some(n)) => format!("CACHE {n}"),
        (true, None) => "CACHE".to_owned(),
    };
    Ok(format!(
        "CREATE SEQUENCE {}\n    AS {}\n    START WITH {}\n    INCREMENT BY {}\n    MINVALUE {}\n    MAXVALUE {}\n    {cycle}\n    {cache};",
        qualified(ns, name),
        text(r, 0),
        text(r, 1),
        text(r, 2),
        text(r, 3),
        text(r, 4)
    ))
}
