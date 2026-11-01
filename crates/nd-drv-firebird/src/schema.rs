use std::collections::BTreeMap;
use std::fmt::Write as _;

use nd_driver::{ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, Object, Relation};

pub(crate) type Rows = Vec<Vec<Option<String>>>;

pub const NAMESPACE: &str = "PUBLIC";

pub(crate) const RELATIONS: &str = "SELECT TRIM(RDB$RELATION_NAME), CASE WHEN RDB$VIEW_BLR IS NULL THEN 0 ELSE 1 END \
FROM RDB$RELATIONS WHERE COALESCE(RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1";

pub(crate) const COLUMNS: &str = "SELECT TRIM(rf.RDB$RELATION_NAME), TRIM(rf.RDB$FIELD_NAME), f.RDB$FIELD_TYPE, \
f.RDB$FIELD_SUB_TYPE, f.RDB$FIELD_PRECISION, f.RDB$FIELD_SCALE, f.RDB$CHARACTER_LENGTH, \
COALESCE(rf.RDB$NULL_FLAG, f.RDB$NULL_FLAG, 0) \
FROM RDB$RELATION_FIELDS rf JOIN RDB$FIELDS f ON f.RDB$FIELD_NAME = rf.RDB$FIELD_SOURCE \
JOIN RDB$RELATIONS r ON r.RDB$RELATION_NAME = rf.RDB$RELATION_NAME \
WHERE COALESCE(r.RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1, rf.RDB$FIELD_POSITION";

pub(crate) const INDEXES: &str = "SELECT TRIM(i.RDB$RELATION_NAME), TRIM(i.RDB$INDEX_NAME), TRIM(s.RDB$FIELD_NAME), \
COALESCE(i.RDB$UNIQUE_FLAG, 0), TRIM(rc.RDB$CONSTRAINT_TYPE) \
FROM RDB$INDICES i JOIN RDB$INDEX_SEGMENTS s ON s.RDB$INDEX_NAME = i.RDB$INDEX_NAME \
LEFT JOIN RDB$RELATION_CONSTRAINTS rc ON rc.RDB$INDEX_NAME = i.RDB$INDEX_NAME \
WHERE COALESCE(i.RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1, 2, s.RDB$FIELD_POSITION";

pub(crate) const FOREIGN_KEYS: &str = "SELECT TRIM(rc.RDB$CONSTRAINT_NAME), TRIM(rc.RDB$RELATION_NAME), TRIM(s.RDB$FIELD_NAME), \
TRIM(ri.RDB$RELATION_NAME), TRIM(si.RDB$FIELD_NAME), TRIM(refc.RDB$UPDATE_RULE), TRIM(refc.RDB$DELETE_RULE) \
FROM RDB$RELATION_CONSTRAINTS rc \
JOIN RDB$REF_CONSTRAINTS refc ON refc.RDB$CONSTRAINT_NAME = rc.RDB$CONSTRAINT_NAME \
JOIN RDB$RELATION_CONSTRAINTS rcu ON rcu.RDB$CONSTRAINT_NAME = refc.RDB$CONST_NAME_UQ \
JOIN RDB$INDEX_SEGMENTS s ON s.RDB$INDEX_NAME = rc.RDB$INDEX_NAME \
JOIN RDB$INDICES ri ON ri.RDB$INDEX_NAME = rcu.RDB$INDEX_NAME \
JOIN RDB$INDEX_SEGMENTS si ON si.RDB$INDEX_NAME = rcu.RDB$INDEX_NAME AND si.RDB$FIELD_POSITION = s.RDB$FIELD_POSITION \
WHERE rc.RDB$CONSTRAINT_TYPE = 'FOREIGN KEY' ORDER BY 1, s.RDB$FIELD_POSITION";

pub(crate) const PROCEDURES: &str = "SELECT TRIM(RDB$PROCEDURE_NAME) FROM RDB$PROCEDURES \
WHERE COALESCE(RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1";

pub(crate) const GENERATORS: &str = "SELECT TRIM(RDB$GENERATOR_NAME) FROM RDB$GENERATORS \
WHERE COALESCE(RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1";

pub(crate) const TRIGGERS: &str = "SELECT TRIM(RDB$TRIGGER_NAME), TRIM(RDB$RELATION_NAME) FROM RDB$TRIGGERS \
WHERE COALESCE(RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1";

fn int(v: Option<&String>) -> i64 {
    v.and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

pub(crate) fn type_name(code: i64, sub: i64, precision: i64, scale: i64, chars: i64) -> String {
    let exact = |base: &str| {
        if scale < 0 {
            let kind = if sub == 2 { "DECIMAL" } else { "NUMERIC" };
            format!("{kind}({precision}, {})", -scale)
        } else {
            base.to_owned()
        }
    };
    match code {
        7 => exact("SMALLINT"),
        8 => exact("INTEGER"),
        16 => exact("BIGINT"),
        26 => exact("INT128"),
        10 => "FLOAT".into(),
        27 => "DOUBLE PRECISION".into(),
        24 => "DECFLOAT(16)".into(),
        25 => "DECFLOAT(34)".into(),
        12 => "DATE".into(),
        13 => "TIME".into(),
        28 => "TIME WITH TIME ZONE".into(),
        35 => "TIMESTAMP".into(),
        29 => "TIMESTAMP WITH TIME ZONE".into(),
        14 => format!("CHAR({chars})"),
        37 => format!("VARCHAR({chars})"),
        23 => "BOOLEAN".into(),
        261 if sub == 1 => "BLOB SUB_TYPE TEXT".into(),
        261 => "BLOB".into(),
        other => format!("TYPE {other}"),
    }
}

fn action(rule: Option<&String>) -> FkAction {
    match rule.map(|s| s.trim().to_ascii_uppercase()).as_deref() {
        Some("CASCADE") => FkAction::Cascade,
        Some("SET NULL") => FkAction::SetNull,
        Some("SET DEFAULT") => FkAction::SetDefault,
        Some("RESTRICT") => FkAction::Restrict,
        _ => FkAction::NoAction,
    }
}

fn s(r: &[Option<String>], i: usize) -> String {
    r.get(i).cloned().flatten().unwrap_or_default()
}

pub(crate) struct Catalog {
    pub relations: Rows,
    pub columns: Rows,
    pub indexes: Rows,
    pub foreign_keys: Rows,
    pub procedures: Rows,
    pub generators: Rows,
    pub triggers: Rows,
}

type Keys = (
    BTreeMap<String, Vec<String>>,
    BTreeMap<(String, String), Index>,
);

fn keys(rows: &Rows) -> Keys {
    let mut pk: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_index: BTreeMap<(String, String), Index> = BTreeMap::new();
    for r in rows {
        let (table, index, column) = (s(r, 0), s(r, 1), s(r, 2));
        match s(r, 4).as_str() {
            "PRIMARY KEY" => pk.entry(table).or_default().push(column),
            "FOREIGN KEY" => {}
            _ => {
                let unique = s(r, 3) == "1";
                by_index
                    .entry((table, index.clone()))
                    .or_insert_with(|| Index {
                        name: index,
                        columns: Vec::new(),
                        unique,
                    })
                    .columns
                    .push(column);
            }
        }
    }
    (pk, by_index)
}

fn add_foreign_keys(tables: &mut BTreeMap<String, Relation>, rows: &Rows) {
    for r in rows {
        let Some(t) = tables.get_mut(&s(r, 1)) else {
            continue;
        };
        let name = s(r, 0);
        if t.foreign_keys.last().is_none_or(|f| f.name != name) {
            t.foreign_keys.push(ForeignKey {
                name,
                columns: Vec::new(),
                ref_namespace: NAMESPACE.into(),
                ref_table: s(r, 3),
                ref_columns: Vec::new(),
                on_update: action(r.get(5).and_then(Option::as_ref)),
                on_delete: action(r.get(6).and_then(Option::as_ref)),
            });
        }
        if let Some(f) = t.foreign_keys.last_mut() {
            f.columns.push(s(r, 2));
            f.ref_columns.push(s(r, 4));
        }
    }
}

pub(crate) fn assemble(c: &Catalog) -> Namespace {
    let mut tables: BTreeMap<String, Relation> = BTreeMap::new();
    let mut views = std::collections::BTreeSet::new();
    for r in &c.relations {
        let name = s(r, 0);
        if s(r, 1) == "1" {
            views.insert(name.clone());
        }
        tables.insert(
            name.clone(),
            Relation {
                name,
                columns: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
                estimated_rows: None,
                detail: None,
            },
        );
    }
    let (pk, by_index) = keys(&c.indexes);
    for r in &c.columns {
        let Some(t) = tables.get_mut(&s(r, 0)) else {
            continue;
        };
        let name = s(r, 1);
        let get = |i: usize| int(r.get(i).and_then(Option::as_ref));
        t.columns.push(ColumnInfo {
            primary_key: pk.get(&t.name).is_some_and(|k| k.contains(&name)),
            type_name: type_name(get(2), get(3), get(4), get(5), get(6)),
            nullable: get(7) == 0,
            name,
        });
    }
    for ((table, _), index) in by_index {
        if let Some(t) = tables.get_mut(&table) {
            t.indexes.push(index);
        }
    }
    add_foreign_keys(&mut tables, &c.foreign_keys);
    let mut ns = Namespace::new(NAMESPACE);
    for (name, rel) in tables {
        if views.contains(&name) {
            ns.views.push(rel);
        } else {
            ns.tables.push(rel);
        }
    }
    ns.functions = c
        .procedures
        .iter()
        .map(|r| Function {
            name: s(r, 0),
            signature: String::new(),
        })
        .collect();
    ns.sequences = c
        .generators
        .iter()
        .map(|r| Object {
            name: s(r, 0),
            detail: None,
        })
        .collect();
    ns.triggers = c
        .triggers
        .iter()
        .map(|r| Object {
            name: s(r, 0),
            detail: r.get(1).cloned().flatten(),
        })
        .collect();
    ns
}

pub(crate) fn create_table(t: &Relation) -> String {
    let q = |n: &str| nd_sql::name(nd_sql::Dialect::Firebird, n);
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
        .map(|c| q(&c.name))
        .collect();
    if !pk.is_empty() {
        lines.push(format!("  PRIMARY KEY ({})", pk.join(", ")));
    }
    let list = |cols: &[String]| cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ");
    for f in &t.foreign_keys {
        let rule = |a: FkAction| match a {
            FkAction::Cascade => " CASCADE",
            FkAction::SetNull => " SET NULL",
            FkAction::SetDefault => " SET DEFAULT",
            FkAction::Restrict | FkAction::NoAction => "",
        };
        let mut line = format!(
            "  CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
            q(&f.name),
            list(&f.columns),
            q(&f.ref_table),
            list(&f.ref_columns)
        );
        if !rule(f.on_update).is_empty() {
            let _ = write!(line, " ON UPDATE{}", rule(f.on_update));
        }
        if !rule(f.on_delete).is_empty() {
            let _ = write!(line, " ON DELETE{}", rule(f.on_delete));
        }
        lines.push(line);
    }
    let mut out = format!("CREATE TABLE {} (\n{}\n);", q(&t.name), lines.join(",\n"));
    for i in &t.indexes {
        let _ = write!(
            out,
            "\n\nCREATE {}INDEX {} ON {} ({});",
            if i.unique { "UNIQUE " } else { "" },
            q(&i.name),
            q(&t.name),
            list(&i.columns)
        );
    }
    out
}

pub(crate) struct IndexFacts {
    pub table: String,
    pub unique: bool,
    pub descending: bool,
    pub expression: Option<String>,
    pub condition: Option<String>,
    pub inactive: bool,
    pub columns: Vec<String>,
    pub constraint: Option<(String, String)>,
}

pub(crate) fn index_ddl(name: &str, f: &IndexFacts) -> String {
    let q = |n: &str| nd_sql::name(nd_sql::Dialect::Firebird, n);
    let cols = f
        .columns
        .iter()
        .map(|c| q(c))
        .collect::<Vec<_>>()
        .join(", ");
    if let Some((constraint, kind)) = f
        .constraint
        .as_ref()
        .filter(|(_, k)| k == "PRIMARY KEY" || k == "UNIQUE")
    {
        return format!(
            "ALTER TABLE {} ADD CONSTRAINT {} {kind} ({cols}) USING INDEX {};",
            q(&f.table),
            q(constraint),
            q(name)
        );
    }
    let body = f.expression.as_deref().map_or_else(
        || format!("({cols})"),
        |e| format!("COMPUTED BY {}", e.trim()),
    );
    let mut out = format!(
        "CREATE {}{}INDEX {} ON {} {body}",
        if f.unique { "UNIQUE " } else { "" },
        if f.descending { "DESCENDING " } else { "" },
        q(name),
        q(&f.table)
    );
    if let Some(c) = f.condition.as_deref().filter(|c| !c.trim().is_empty()) {
        out.push(' ');
        out.push_str(c.trim());
    }
    out.push(';');
    if f.inactive {
        let _ = write!(out, "\nALTER INDEX {} INACTIVE;", q(name));
    }
    out
}

pub(crate) const INDEX_FACTS: &str = "SELECT TRIM(i.RDB$RELATION_NAME), COALESCE(i.RDB$UNIQUE_FLAG, 0), \
COALESCE(i.RDB$INDEX_TYPE, 0), i.RDB$EXPRESSION_SOURCE, COALESCE(i.RDB$INDEX_INACTIVE, 0), \
TRIM(rc.RDB$CONSTRAINT_NAME), TRIM(rc.RDB$CONSTRAINT_TYPE) \
FROM RDB$INDICES i LEFT JOIN RDB$RELATION_CONSTRAINTS rc ON rc.RDB$INDEX_NAME = i.RDB$INDEX_NAME \
WHERE i.RDB$INDEX_NAME = ?";

pub(crate) const INDEX_CONDITION: &str =
    "SELECT RDB$CONDITION_SOURCE FROM RDB$INDICES WHERE RDB$INDEX_NAME = ?";

pub(crate) const INDEX_SEGMENTS: &str = "SELECT TRIM(RDB$FIELD_NAME) FROM RDB$INDEX_SEGMENTS \
WHERE RDB$INDEX_NAME = ? ORDER BY RDB$FIELD_POSITION";

pub(crate) const DEPENDENCIES: &str = "SELECT 'uses', 'definition', \
CASE d.RDB$DEPENDED_ON_TYPE WHEN 0 THEN 'table' WHEN 1 THEN 'view' WHEN 2 THEN 'trigger' \
  WHEN 5 THEN 'routine' WHEN 15 THEN 'routine' WHEN 14 THEN 'sequence' END, \
TRIM(d.RDB$DEPENDED_ON_NAME), CAST(NULL AS VARCHAR(63)) \
FROM RDB$DEPENDENCIES d WHERE d.RDB$DEPENDENT_NAME = ? AND d.RDB$DEPENDED_ON_NAME <> d.RDB$DEPENDENT_NAME \
UNION \
SELECT 'used_by', CASE WHEN d.RDB$DEPENDENT_TYPE = 2 THEN 'trigger' ELSE 'definition' END, \
CASE d.RDB$DEPENDENT_TYPE WHEN 0 THEN 'table' WHEN 1 THEN 'view' WHEN 2 THEN 'trigger' \
  WHEN 5 THEN 'routine' WHEN 15 THEN 'routine' END, \
TRIM(d.RDB$DEPENDENT_NAME), CAST(NULL AS VARCHAR(63)) \
FROM RDB$DEPENDENCIES d WHERE d.RDB$DEPENDED_ON_NAME = ? AND d.RDB$DEPENDED_ON_NAME <> d.RDB$DEPENDENT_NAME \
UNION \
SELECT 'uses', 'foreign_key', 'table', TRIM(ri.RDB$RELATION_NAME), TRIM(rc.RDB$CONSTRAINT_NAME) \
FROM RDB$RELATION_CONSTRAINTS rc \
JOIN RDB$REF_CONSTRAINTS refc ON refc.RDB$CONSTRAINT_NAME = rc.RDB$CONSTRAINT_NAME \
JOIN RDB$RELATION_CONSTRAINTS ru ON ru.RDB$CONSTRAINT_NAME = refc.RDB$CONST_NAME_UQ \
JOIN RDB$INDICES ri ON ri.RDB$INDEX_NAME = ru.RDB$INDEX_NAME \
WHERE rc.RDB$CONSTRAINT_TYPE = 'FOREIGN KEY' AND rc.RDB$RELATION_NAME = ? \
UNION \
SELECT 'used_by', 'foreign_key', 'table', TRIM(rc.RDB$RELATION_NAME), TRIM(rc.RDB$CONSTRAINT_NAME) \
FROM RDB$RELATION_CONSTRAINTS rc \
JOIN RDB$REF_CONSTRAINTS refc ON refc.RDB$CONSTRAINT_NAME = rc.RDB$CONSTRAINT_NAME \
JOIN RDB$RELATION_CONSTRAINTS ru ON ru.RDB$CONSTRAINT_NAME = refc.RDB$CONST_NAME_UQ \
WHERE rc.RDB$CONSTRAINT_TYPE = 'FOREIGN KEY' AND ru.RDB$RELATION_NAME = ?";

pub(crate) fn dependency(r: &[Option<String>]) -> Option<nd_driver::Dependency> {
    let cell = |i: usize| r.get(i).cloned().flatten().map(|v| v.trim().to_owned());
    nd_driver::Dependency::from_row(&[
        cell(0),
        cell(1),
        cell(2),
        Some(NAMESPACE.to_owned()),
        cell(3),
        cell(4),
    ])
}
