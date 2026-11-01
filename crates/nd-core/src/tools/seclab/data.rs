use std::collections::BTreeMap;

use super::{Ctx, Outcome, SecSeverity, SecSkip, cell, found, on, rows, subject};
use crate::tools::Engine;

const SAMPLE: usize = 5;
const MAX_SAMPLED: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    TaxCode,
    Iban,
    Email,
    Phone,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Self::TaxCode => "cf",
            Self::Iban => "iban",
            Self::Email => "email",
            Self::Phone => "phone",
        }
    }
}

fn kind_of_name(column: &str) -> Option<Kind> {
    let n: String = column
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&[
        "codicefiscale",
        "codfisc",
        "fiscalcode",
        "taxcode",
        "cfiscale",
    ]) || n == "cf"
        || n == "ssn"
    {
        Some(Kind::TaxCode)
    } else if n.contains("iban") {
        Some(Kind::Iban)
    } else if has(&["email"])
        || n == "mail"
        || n == "pec"
        || n.ends_with("mail") && !n.ends_with("gmail")
    {
        Some(Kind::Email)
    } else if has(&["telefono", "phone", "cellulare", "mobile", "telephone"])
        || matches!(n.as_str(), "tel" | "cell" | "cel")
    {
        Some(Kind::Phone)
    } else {
        None
    }
}

fn tax_code(v: &str) -> bool {
    let c: Vec<char> = v.trim().to_uppercase().chars().collect();
    if c.len() != 16 {
        return false;
    }
    let digit_like = |ch: char| ch.is_ascii_digit() || "LMNPQRSTUV".contains(ch);
    c.iter().enumerate().all(|(i, ch)| match i {
        0..=5 | 8 | 11 | 15 => ch.is_ascii_uppercase(),
        _ => digit_like(*ch),
    })
}

fn iban(v: &str) -> bool {
    let s: String = v
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    if !(15..=34).contains(&s.len())
        || !s.is_char_boundary(4)
        || !s.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return false;
    }
    let (head, tail) = s.split_at(4);
    if !head[..2].chars().all(|c| c.is_ascii_uppercase())
        || !head[2..].chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    let mut rem: u32 = 0;
    for c in tail.chars().chain(head.chars()) {
        let Some(d) = c.to_digit(36) else {
            return false;
        };
        rem = if d < 10 {
            (rem * 10 + d) % 97
        } else {
            (rem * 100 + d) % 97
        };
    }
    rem == 1
}

fn email(v: &str) -> bool {
    let v = v.trim();
    let Some((local, domain)) = v.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.contains('@')
        && !v.contains(char::is_whitespace)
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

fn phone(v: &str) -> bool {
    let body = v.trim().strip_prefix('+').unwrap_or(v.trim());
    let digits = body.chars().filter(char::is_ascii_digit).count();
    (7..=15).contains(&digits)
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || " .-()/".contains(c))
}

fn matches(kind: Kind, v: &str) -> bool {
    match kind {
        Kind::TaxCode => tax_code(v),
        Kind::Iban => iban(v),
        Kind::Email => email(v),
        Kind::Phone => phone(v),
    }
}

struct Column {
    schema: String,
    table: String,
    name: String,
    protected: bool,
}

fn columns_sql(engine: Engine) -> &'static str {
    match engine {
        Engine::Postgres => {
            "SELECT n.nspname, c.relname, a.attname, (c.relrowsecurity OR format_type(a.atttypid, a.atttypmod) = 'bytea')::text \
FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
WHERE c.relkind IN ('r', 'p') AND a.attnum > 0 AND NOT a.attisdropped AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
AND n.nspname NOT LIKE 'pg\\_%' ORDER BY 1, 2, a.attnum LIMIT 30000"
        }
        Engine::Cockroach => {
            "SELECT table_schema, table_name, column_name, (data_type = 'bytes')::STRING FROM information_schema.columns \
WHERE table_schema NOT IN ('crdb_internal', 'information_schema', 'pg_catalog', 'pg_extension') ORDER BY 1, 2, ordinal_position LIMIT 30000"
        }
        Engine::Mysql | Engine::Mariadb => {
            "SELECT c.TABLE_SCHEMA, c.TABLE_NAME, c.COLUMN_NAME, \
(c.DATA_TYPE IN ('varbinary', 'binary', 'blob') OR UPPER(t.CREATE_OPTIONS) LIKE '%ENCRYPT%Y%') \
FROM information_schema.COLUMNS c JOIN information_schema.TABLES t ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME \
WHERE t.TABLE_TYPE = 'BASE TABLE' AND c.TABLE_SCHEMA NOT IN ('mysql', 'sys', 'information_schema', 'performance_schema') \
ORDER BY 1, 2, c.ORDINAL_POSITION LIMIT 30000"
        }
        Engine::Mssql => {
            "SELECT TOP 30000 s.name, t.name, c.name, CASE WHEN c.encryption_type IS NOT NULL OR c.is_masked = 1 \
OR EXISTS (SELECT 1 FROM sys.security_predicates p WHERE p.target_object_id = t.object_id) \
OR ty.name IN ('varbinary', 'binary') THEN 1 ELSE 0 END \
FROM sys.columns c JOIN sys.tables t ON t.object_id = c.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
JOIN sys.types ty ON ty.user_type_id = c.user_type_id WHERE t.is_ms_shipped = 0 ORDER BY s.name, t.name, c.column_id"
        }
        Engine::Clickhouse => {
            "SELECT database, table, name, '0' FROM system.columns \
WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema') ORDER BY 1, 2, position LIMIT 30000"
        }
        Engine::Duckdb | Engine::Mongo => {
            "SELECT table_schema, table_name, column_name, (data_type = 'BLOB')::VARCHAR FROM information_schema.columns \
WHERE table_schema NOT IN ('information_schema', 'pg_catalog') ORDER BY 1, 2, ordinal_position LIMIT 30000"
        }
        Engine::Oracle => {
            "SELECT c.owner, c.table_name, c.column_name, CASE WHEN c.data_type IN ('RAW', 'BLOB', 'LONG RAW') THEN 1 ELSE 0 END \
FROM all_tab_columns c JOIN all_tables t ON t.owner = c.owner AND t.table_name = c.table_name \
JOIN all_users u ON u.username = c.owner AND u.oracle_maintained = 'N' ORDER BY 1, 2, c.column_id FETCH FIRST 30000 ROWS ONLY"
        }
        Engine::Db2 => {
            "SELECT RTRIM(c.TABSCHEMA), RTRIM(c.TABNAME), c.COLNAME, CASE WHEN c.TYPENAME IN ('BLOB', 'VARBINARY', 'BINARY') THEN 1 ELSE 0 END \
FROM SYSCAT.COLUMNS c JOIN SYSCAT.TABLES t ON t.TABSCHEMA = c.TABSCHEMA AND t.TABNAME = c.TABNAME AND t.TYPE = 'T' \
WHERE c.TABSCHEMA NOT LIKE 'SYS%' ORDER BY 1, 2, c.COLNO FETCH FIRST 30000 ROWS ONLY"
        }
        Engine::Hana => {
            "SELECT SCHEMA_NAME, TABLE_NAME, COLUMN_NAME, CASE WHEN DATA_TYPE_NAME IN ('BLOB', 'VARBINARY', 'BINARY') THEN 1 ELSE 0 END \
FROM SYS.TABLE_COLUMNS WHERE SCHEMA_NAME IN (SELECT SCHEMA_NAME FROM SYS.SCHEMAS WHERE SCHEMA_OWNER <> 'SYS' \
AND SCHEMA_OWNER NOT LIKE '\\_SYS%' ESCAPE '\\') ORDER BY 1, 2, POSITION LIMIT 30000"
        }
        Engine::Odbc => {
            "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, 0 FROM INFORMATION_SCHEMA.COLUMNS ORDER BY 1, 2, ORDINAL_POSITION"
        }
        Engine::Firebird => {
            "SELECT 'PUBLIC', TRIM(rf.RDB$RELATION_NAME), TRIM(rf.RDB$FIELD_NAME), \
CASE WHEN f.RDB$FIELD_TYPE = 261 AND f.RDB$FIELD_SUB_TYPE <> 1 THEN 1 ELSE 0 END \
FROM RDB$RELATION_FIELDS rf JOIN RDB$FIELDS f ON f.RDB$FIELD_NAME = rf.RDB$FIELD_SOURCE \
JOIN RDB$RELATIONS r ON r.RDB$RELATION_NAME = rf.RDB$RELATION_NAME \
WHERE COALESCE(r.RDB$SYSTEM_FLAG, 0) = 0 AND r.RDB$VIEW_BLR IS NULL ORDER BY 2, rf.RDB$FIELD_POSITION ROWS 30000"
        }
        Engine::Sqlite | Engine::Libsql => {
            "SELECT 'main', m.name, p.name, CASE WHEN upper(p.type) LIKE '%BLOB%' THEN 1 ELSE 0 END \
FROM sqlite_master m JOIN pragma_table_info(m.name) p WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\' LIMIT 30000"
        }
    }
}

fn target(cx: &Ctx<'_>, c: &Column) -> String {
    match cx.engine {
        Engine::Sqlite | Engine::Libsql => cx.q(&c.table),
        _ => format!("{}.{}", cx.q(&c.schema), cx.q(&c.table)),
    }
}

async fn confirmed(cx: &Ctx<'_>, c: &Column, kind: Kind) -> bool {
    let (col, table) = (cx.q(&c.name), target(cx, c));
    let sql = if cx.engine == Engine::Mssql {
        format!(
            "SELECT TOP {SAMPLE} CAST({col} AS nvarchar(200)) FROM {table} WHERE {col} IS NOT NULL"
        )
    } else {
        format!(
            "SELECT CAST({col} AS {}) FROM {table} WHERE {col} IS NOT NULL LIMIT {SAMPLE}",
            if cx.engine == Engine::Mysql || cx.engine == Engine::Mariadb {
                "CHAR"
            } else {
                "VARCHAR"
            }
        )
    };
    rows(cx.conn, &sql)
        .await
        .is_some_and(|r| r.iter().any(|row| matches(kind, &cell(row, 0))))
}

fn fix_for(cx: &Ctx<'_>, tables: &BTreeMap<(String, String), Vec<(String, Kind)>>) -> Vec<String> {
    let mut out = Vec::new();
    for ((schema, table), cols) in tables {
        let c = Column {
            schema: schema.clone(),
            table: table.clone(),
            name: String::new(),
            protected: false,
        };
        let t = target(cx, &c);
        match cx.engine {
            Engine::Postgres => {
                out.push(format!("ALTER TABLE {t} ENABLE ROW LEVEL SECURITY;\n-- then a policy for the roles that must see the rows: CREATE POLICY name ON {t} FOR SELECT TO role USING (...);"));
            }
            Engine::Mysql => out.push(format!("ALTER TABLE {t} ENCRYPTION = 'Y';")),
            Engine::Mariadb => out.push(format!("ALTER TABLE {t} ENCRYPTED = YES;")),
            Engine::Mssql => {
                for (name, kind) in cols {
                    let f = if *kind == Kind::Email { "email()" } else { "default()" };
                    out.push(format!("ALTER TABLE {t} ALTER COLUMN {} ADD MASKED WITH (FUNCTION = '{f}');", cx.q(name)));
                }
            }
            Engine::Clickhouse => {
                let names: Vec<String> = cols.iter().map(|(n, _)| cx.q(n)).collect();
                out.push(format!("REVOKE SELECT({}) ON {t} FROM ALL;", names.join(", ")));
            }
            Engine::Cockroach => out.push(format!("-- {t}: encrypt the columns in the application and grant SELECT only to the roles that need it (REVOKE SELECT ON TABLE {t} FROM role;)")),
            _ => out.push(format!("-- {t}: the file is not encrypted; encrypt it (SQLCipher, or the engine's own encryption) and restrict access to it")),
        }
    }
    out
}

pub(super) async fn sensitive(cx: &Ctx<'_>) -> Outcome {
    let Some(listed) = rows(cx.conn, columns_sql(cx.engine)).await else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let columns: Vec<(Column, Kind)> = listed
        .iter()
        .filter_map(|r| {
            let kind = kind_of_name(&cell(r, 2))?;
            Some((
                Column {
                    schema: cell(r, 0),
                    table: cell(r, 1),
                    name: cell(r, 2),
                    protected: on(r, 3),
                },
                kind,
            ))
        })
        .collect();
    let open: Vec<&(Column, Kind)> = columns.iter().filter(|(c, _)| !c.protected).collect();
    if open.is_empty() {
        return Outcome::Passed;
    }
    let mut subjects = Vec::new();
    let mut tables: BTreeMap<(String, String), Vec<(String, Kind)>> = BTreeMap::new();
    let mut severity = SecSeverity::Low;
    for (i, (c, kind)) in open.iter().enumerate() {
        let sure = i < MAX_SAMPLED && confirmed(cx, c, *kind).await;
        if sure {
            severity = SecSeverity::Medium;
        }
        let name = match cx.engine {
            Engine::Sqlite | Engine::Libsql => format!("{}.{}", c.table, c.name),
            _ => format!("{}.{}.{}", c.schema, c.table, c.name),
        };
        subjects.push(subject(
            name,
            Some(format!(
                "{}{}",
                kind.tag(),
                if sure { "+values" } else { "" }
            )),
        ));
        tables
            .entry((c.schema.clone(), c.table.clone()))
            .or_default()
            .push((c.name.clone(), *kind));
    }
    found(severity, subjects, fix_for(cx, &tables))
}
