use nd_driver::{CatalogRows, FkAction, Relation};
use std::fmt::Write as _;

fn user(column: &str) -> String {
    format!("{column} NOT LIKE 'SYS%' AND {column} NOT IN ('NULLID', 'SQLJ')")
}

pub fn relations() -> String {
    format!(
        "SELECT RTRIM(TABSCHEMA), RTRIM(TABNAME), CASE TYPE WHEN 'V' THEN 'view' WHEN 'S' THEN 'materialized' \
ELSE 'table' END, CARD FROM SYSCAT.TABLES WHERE TYPE IN ('T', 'V', 'S', 'G') AND {} ORDER BY 1, 2",
        user("TABSCHEMA")
    )
}

pub fn columns_sql() -> String {
    format!(
        "SELECT RTRIM(TABSCHEMA), RTRIM(TABNAME), COLNAME, RTRIM(TYPENAME), LENGTH, SCALE, NULLS \
FROM SYSCAT.COLUMNS WHERE {} ORDER BY TABSCHEMA, TABNAME, COLNO",
        user("TABSCHEMA")
    )
}

pub fn indexes() -> String {
    format!(
        "SELECT RTRIM(i.TABSCHEMA), RTRIM(i.TABNAME), RTRIM(i.INDNAME), u.COLNAME, \
CASE WHEN i.UNIQUERULE IN ('U', 'P') THEN 'Y' ELSE 'N' END, CASE WHEN i.UNIQUERULE = 'P' THEN 'Y' ELSE 'N' END \
FROM SYSCAT.INDEXES i JOIN SYSCAT.INDEXCOLUSE u ON u.INDSCHEMA = i.INDSCHEMA AND u.INDNAME = i.INDNAME \
WHERE {} ORDER BY 1, 2, 3, u.COLSEQ",
        user("i.TABSCHEMA")
    )
}

pub fn foreign_keys() -> String {
    format!(
        "SELECT RTRIM(r.TABSCHEMA), RTRIM(r.TABNAME), RTRIM(r.CONSTNAME), RTRIM(r.REFTABSCHEMA), RTRIM(r.REFTABNAME), \
fk.COLNAME, pk.COLNAME, \
CASE r.DELETERULE WHEN 'C' THEN 'CASCADE' WHEN 'N' THEN 'SET NULL' WHEN 'R' THEN 'RESTRICT' ELSE 'NO ACTION' END, \
CASE r.UPDATERULE WHEN 'R' THEN 'RESTRICT' ELSE 'NO ACTION' END \
FROM SYSCAT.REFERENCES r \
JOIN SYSCAT.KEYCOLUSE fk ON fk.CONSTNAME = r.CONSTNAME AND fk.TABSCHEMA = r.TABSCHEMA AND fk.TABNAME = r.TABNAME \
JOIN SYSCAT.KEYCOLUSE pk ON pk.CONSTNAME = r.REFKEYNAME AND pk.TABSCHEMA = r.REFTABSCHEMA \
  AND pk.TABNAME = r.REFTABNAME AND pk.COLSEQ = fk.COLSEQ \
WHERE {} ORDER BY 1, 2, 3, fk.COLSEQ",
        user("r.TABSCHEMA")
    )
}

pub fn routines() -> String {
    format!(
        "SELECT RTRIM(ROUTINESCHEMA), ROUTINENAME, SPECIFICNAME FROM SYSCAT.ROUTINES \
WHERE ORIGIN IN ('Q', 'E') AND {} ORDER BY 1, 2",
        user("ROUTINESCHEMA")
    )
}

pub fn sequences() -> String {
    format!(
        "SELECT RTRIM(SEQSCHEMA), SEQNAME FROM SYSCAT.SEQUENCES WHERE SEQTYPE = 'S' AND {} ORDER BY 1, 2",
        user("SEQSCHEMA")
    )
}

pub fn triggers() -> String {
    format!(
        "SELECT RTRIM(TRIGSCHEMA), TRIGNAME, TABNAME FROM SYSCAT.TRIGGERS WHERE {} ORDER BY 1, 2",
        user("TRIGSCHEMA")
    )
}

pub const CURRENT_SCHEMA: &str = "SELECT RTRIM(CURRENT SCHEMA) FROM SYSIBM.SYSDUMMY1";

pub const VERSION: &str = "SELECT SERVICE_LEVEL FROM TABLE(SYSPROC.ENV_GET_INST_INFO())";

pub const VIEW_TEXT: &str = "SELECT TEXT FROM SYSCAT.VIEWS WHERE VIEWSCHEMA = ? AND VIEWNAME = ?";

pub const ROUTINE_TEXT: &str = "SELECT TEXT FROM SYSCAT.ROUTINES WHERE ROUTINESCHEMA = ? AND (SPECIFICNAME = ? OR ROUTINENAME = ?)";

pub const TRIGGER_TEXT: &str =
    "SELECT TEXT FROM SYSCAT.TRIGGERS WHERE TRIGSCHEMA = ? AND TRIGNAME = ?";

pub const SEQUENCE_FACTS: &str = "SELECT START, INCREMENT, MINVALUE, MAXVALUE, CYCLE, CACHE \
FROM SYSCAT.SEQUENCES WHERE SEQSCHEMA = ? AND SEQNAME = ?";

pub const INDEX_FACTS: &str = "SELECT RTRIM(TABSCHEMA), RTRIM(TABNAME), UNIQUERULE \
FROM SYSCAT.INDEXES WHERE INDSCHEMA = ? AND INDNAME = ?";

pub const INDEX_COLUMNS: &str = "SELECT COLNAME, COLORDER FROM SYSCAT.INDEXCOLUSE WHERE INDSCHEMA = ? AND INDNAME = ? ORDER BY COLSEQ";

pub const DEPENDENCIES: &str = "SELECT 'used_by', 'definition', \
CASE d.DTYPE WHEN 'V' THEN 'view' WHEN 'S' THEN 'view' WHEN 'F' THEN 'routine' WHEN 'B' THEN 'trigger' ELSE 'table' END, \
RTRIM(d.TABSCHEMA), RTRIM(d.TABNAME), CAST(NULL AS VARCHAR(128)) FROM SYSCAT.TABDEP d WHERE d.BSCHEMA = ? AND d.BNAME = ? \
UNION ALL SELECT 'uses', 'definition', \
CASE d.BTYPE WHEN 'V' THEN 'view' WHEN 'S' THEN 'view' WHEN 'F' THEN 'routine' WHEN 'Q' THEN 'sequence' ELSE 'table' END, \
RTRIM(d.BSCHEMA), RTRIM(d.BNAME), CAST(NULL AS VARCHAR(128)) FROM SYSCAT.TABDEP d WHERE d.TABSCHEMA = ? AND d.TABNAME = ? \
UNION ALL SELECT 'used_by', 'trigger', 'trigger', RTRIM(t.TRIGSCHEMA), RTRIM(t.TRIGNAME), CAST(NULL AS VARCHAR(128)) \
FROM SYSCAT.TRIGGERS t WHERE t.TABSCHEMA = ? AND t.TABNAME = ? \
UNION ALL SELECT 'uses', 'foreign_key', 'table', RTRIM(r.REFTABSCHEMA), RTRIM(r.REFTABNAME), RTRIM(r.CONSTNAME) \
FROM SYSCAT.REFERENCES r WHERE r.TABSCHEMA = ? AND r.TABNAME = ? \
UNION ALL SELECT 'used_by', 'foreign_key', 'table', RTRIM(r.TABSCHEMA), RTRIM(r.TABNAME), RTRIM(r.CONSTNAME) \
FROM SYSCAT.REFERENCES r WHERE r.REFTABSCHEMA = ? AND r.REFTABNAME = ?";

pub fn type_name(typename: &str, length: &str, scale: &str) -> String {
    let t = typename.trim().to_ascii_uppercase();
    let (len, sc) = (length.trim(), scale.trim());
    match t.as_str() {
        "VARCHAR" | "VARGRAPHIC" | "VARBINARY" | "BINARY" | "GRAPHIC" | "CLOB" | "BLOB"
        | "DBCLOB" => format!("{t}({len})"),
        "CHARACTER" => format!("CHAR({len})"),
        "DECIMAL" => format!("DECIMAL({len},{sc})"),
        "DECFLOAT" => if len == "8" {
            "DECFLOAT(16)"
        } else {
            "DECFLOAT(34)"
        }
        .into(),
        "TIMESTAMP" if !sc.is_empty() && sc != "6" => format!("TIMESTAMP({sc})"),
        _ => t,
    }
}

pub fn columns(raw: Vec<Vec<Option<String>>>) -> Vec<Vec<Option<String>>> {
    raw.into_iter()
        .map(|r| {
            let cell = |i: usize| r.get(i).cloned().flatten().unwrap_or_default();
            vec![
                Some(cell(0)),
                Some(cell(1)),
                Some(cell(2)),
                Some(type_name(&cell(3), &cell(4), &cell(5))),
                Some(cell(6)),
            ]
        })
        .collect()
}

pub fn catalog_sql() -> [String; 7] {
    [
        relations(),
        columns_sql(),
        indexes(),
        foreign_keys(),
        routines(),
        sequences(),
        triggers(),
    ]
}

pub fn catalog(mut answers: Vec<Vec<Vec<Option<String>>>>) -> CatalogRows {
    let mut take = || {
        if answers.is_empty() {
            Vec::new()
        } else {
            answers.remove(0)
        }
    };
    CatalogRows {
        relations: take(),
        columns: columns(take()),
        indexes: take(),
        foreign_keys: take(),
        routines: take(),
        sequences: take(),
        triggers: take(),
    }
}

fn q(n: &str) -> String {
    nd_sql::name(nd_sql::Dialect::Db2, n)
}

pub fn create_table(dialect: nd_sql::Dialect, ns: &str, t: &Relation) -> String {
    let q = |n: &str| nd_sql::name(dialect, n);
    let table = format!("{}.{}", q(ns), q(&t.name));
    let list = |cols: &[String]| cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ");
    let mut lines: Vec<String> = t
        .columns
        .iter()
        .map(|c| {
            format!(
                "  {} {}{}",
                q(&c.name),
                c.type_name,
                if c.nullable { "" } else { " NOT NULL" }
            )
        })
        .collect();
    let pk: Vec<String> = t
        .columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.clone())
        .collect();
    if !pk.is_empty() {
        lines.push(format!("  PRIMARY KEY ({})", list(&pk)));
    }
    for f in &t.foreign_keys {
        let mut line = format!(
            "  CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}.{} ({})",
            q(&f.name),
            list(&f.columns),
            q(&f.ref_namespace),
            q(&f.ref_table),
            list(&f.ref_columns)
        );
        let rule = match f.on_delete {
            FkAction::Cascade => " ON DELETE CASCADE",
            FkAction::SetNull => " ON DELETE SET NULL",
            FkAction::Restrict => " ON DELETE RESTRICT",
            FkAction::SetDefault | FkAction::NoAction => "",
        };
        line.push_str(rule);
        lines.push(line);
    }
    let mut out = format!("CREATE TABLE {table} (\n{}\n);", lines.join(",\n"));
    for i in t.indexes.iter().filter(|i| {
        let mut cols = i.columns.clone();
        cols.sort();
        let mut keys = pk.clone();
        keys.sort();
        cols != keys
    }) {
        let index = if dialect == nd_sql::Dialect::Db2 {
            format!("{}.{}", q(ns), q(&i.name))
        } else {
            q(&i.name)
        };
        let _ = write!(
            out,
            "\n\nCREATE {}INDEX {index} ON {table} ({});",
            if i.unique { "UNIQUE " } else { "" },
            list(&i.columns)
        );
    }
    out
}

pub fn index_ddl(
    schema: &str,
    name: &str,
    table_schema: &str,
    table: &str,
    unique_rule: &str,
    columns: &[(String, String)],
) -> String {
    let keyed: Vec<String> = columns
        .iter()
        .filter(|(_, o)| o != "I")
        .map(|(c, o)| {
            if o == "D" {
                format!("{} DESC", q(c))
            } else {
                q(c)
            }
        })
        .collect();
    let included: Vec<String> = columns
        .iter()
        .filter(|(_, o)| o == "I")
        .map(|(c, _)| q(c))
        .collect();
    let mut out = format!(
        "CREATE {}INDEX {}.{} ON {}.{} ({})",
        if matches!(unique_rule.trim(), "U" | "P") {
            "UNIQUE "
        } else {
            ""
        },
        q(schema),
        q(name),
        q(table_schema),
        q(table),
        keyed.join(", ")
    );
    if !included.is_empty() {
        let _ = write!(out, " INCLUDE ({})", included.join(", "));
    }
    out.push(';');
    out
}

pub fn sequence_ddl(schema: &str, name: &str, facts: &[Option<String>]) -> String {
    let cell = |i: usize| facts.get(i).cloned().flatten().unwrap_or_default();
    let cache = cell(5);
    format!(
        "CREATE SEQUENCE {}.{} START WITH {} INCREMENT BY {} MINVALUE {} MAXVALUE {}{}{};",
        q(schema),
        q(name),
        cell(0),
        cell(1),
        cell(2),
        cell(3),
        if cell(4) == "Y" {
            " CYCLE"
        } else {
            " NO CYCLE"
        },
        if cache.is_empty() || cache == "0" {
            " NO CACHE".to_owned()
        } else {
            format!(" CACHE {cache}")
        }
    )
}
