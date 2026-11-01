use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
pub struct Schema {
    pub databases: Vec<String>,
    pub current_database: String,
    pub namespaces: Vec<Namespace>,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
pub struct Outline {
    pub databases: Vec<String>,
    pub current_database: String,
    pub namespaces: Vec<NamespaceSize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct NamespaceSize {
    pub name: String,
    #[specta(type = f64)]
    pub relations: u64,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Namespace {
    pub name: String,
    pub tables: Vec<Relation>,
    pub views: Vec<Relation>,
    pub functions: Vec<Function>,
    pub sequences: Vec<Object>,
    pub types: Vec<Object>,
    pub triggers: Vec<Object>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub materialized_views: Vec<Relation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub dictionaries: Vec<Relation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub packages: Vec<Object>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub synonyms: Vec<Object>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub aliases: Vec<Relation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub data_streams: Vec<Relation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub managed: Option<crate::Flavor>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    #[specta(optional)]
    pub tags: std::collections::BTreeMap<String, Vec<crate::RelationTag>>,
}

impl Namespace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tables: Vec::new(),
            views: Vec::new(),
            functions: Vec::new(),
            sequences: Vec::new(),
            types: Vec::new(),
            triggers: Vec::new(),
            materialized_views: Vec::new(),
            dictionaries: Vec::new(),
            packages: Vec::new(),
            synonyms: Vec::new(),
            aliases: Vec::new(),
            data_streams: Vec::new(),
            managed: None,
            tags: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Object {
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Relation {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<Index>,
    pub foreign_keys: Vec<ForeignKey>,
    pub estimated_rows: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Function {
    pub name: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_namespace: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: FkAction,
    pub on_update: FkAction,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SchemaStart {
    pub schema: PackedSchema,
    pub pending: Vec<NamespaceSize>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct PackedSchema {
    pub databases: Vec<String>,
    pub current_database: String,
    pub namespaces: Vec<PackedNamespace>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct PackedNamespace {
    pub name: String,
    pub tables: Vec<PackedRelation>,
    pub views: Vec<PackedRelation>,
    pub functions: Vec<Function>,
    pub sequences: Vec<Object>,
    pub types: Vec<Object>,
    pub triggers: Vec<Object>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub materialized_views: Vec<PackedRelation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub dictionaries: Vec<PackedRelation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub packages: Vec<Object>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub synonyms: Vec<Object>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub aliases: Vec<PackedRelation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[specta(optional)]
    pub data_streams: Vec<PackedRelation>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    #[specta(optional)]
    pub details: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub managed: Option<crate::Flavor>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    #[specta(optional)]
    pub tags: std::collections::BTreeMap<String, Vec<crate::RelationTag>>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct PackedRelation(
    pub String,
    pub String,
    pub String,
    pub String,
    pub Option<f64>,
);

fn escape_into(out: &mut String, s: &str) {
    if !s.contains(['\\', '\t', '\n']) {
        out.push_str(s);
        return;
    }
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
}

fn lines<T>(items: &[T], fields: impl Fn(&T, &mut Vec<String>)) -> String {
    let mut out = String::new();
    let mut row = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        row.clear();
        fields(item, &mut row);
        for (j, f) in row.iter().enumerate() {
            if j > 0 {
                out.push('\t');
            }
            escape_into(&mut out, f);
        }
    }
    out
}

fn action(a: FkAction) -> &'static str {
    match a {
        FkAction::NoAction => "no_action",
        FkAction::Restrict => "restrict",
        FkAction::Cascade => "cascade",
        FkAction::SetNull => "set_null",
        FkAction::SetDefault => "set_default",
    }
}

impl From<Relation> for PackedRelation {
    fn from(r: Relation) -> Self {
        let mut columns = String::with_capacity(r.columns.len() * 24);
        for (i, c) in r.columns.iter().enumerate() {
            if i > 0 {
                columns.push('\n');
            }
            escape_into(&mut columns, &c.name);
            columns.push('\t');
            escape_into(&mut columns, &c.type_name);
            columns.push('\t');
            columns.push(char::from(
                b'0' + u8::from(c.nullable) + 2 * u8::from(c.primary_key),
            ));
        }
        let indexes = lines(&r.indexes, |i, row| {
            row.push(i.name.clone());
            row.push(if i.unique { "1" } else { "0" }.into());
            row.extend(i.columns.iter().cloned());
        });
        let foreign_keys = lines(&r.foreign_keys, |k, row| {
            row.extend([
                k.name.clone(),
                k.ref_namespace.clone(),
                k.ref_table.clone(),
                action(k.on_delete).into(),
                action(k.on_update).into(),
            ]);
            for (c, rc) in k.columns.iter().zip(&k.ref_columns) {
                row.extend([c.clone(), rc.clone()]);
            }
        });
        Self(r.name, columns, indexes, foreign_keys, r.estimated_rows)
    }
}

impl From<Schema> for PackedSchema {
    fn from(s: Schema) -> Self {
        let pack = |list: Vec<Relation>| list.into_iter().map(PackedRelation::from).collect();
        Self {
            databases: s.databases,
            current_database: s.current_database,
            namespaces: s
                .namespaces
                .into_iter()
                .map(|n| {
                    let details = n
                        .tables
                        .iter()
                        .chain(&n.views)
                        .chain(&n.materialized_views)
                        .chain(&n.dictionaries)
                        .chain(&n.aliases)
                        .chain(&n.data_streams)
                        .filter_map(|r| r.detail.clone().map(|d| (r.name.clone(), d)))
                        .collect();
                    PackedNamespace {
                        name: n.name,
                        tables: pack(n.tables),
                        views: pack(n.views),
                        functions: n.functions,
                        sequences: n.sequences,
                        types: n.types,
                        triggers: n.triggers,
                        materialized_views: pack(n.materialized_views),
                        dictionaries: pack(n.dictionaries),
                        packages: n.packages,
                        synonyms: n.synonyms,
                        aliases: pack(n.aliases),
                        data_streams: pack(n.data_streams),
                        details,
                        managed: n.managed,
                        tags: n.tags,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FkAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

impl FkAction {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "RESTRICT" | "R" => Self::Restrict,
            "CASCADE" | "C" => Self::Cascade,
            "SET NULL" | "N" => Self::SetNull,
            "SET DEFAULT" | "D" => Self::SetDefault,
            _ => Self::NoAction,
        }
    }
}
