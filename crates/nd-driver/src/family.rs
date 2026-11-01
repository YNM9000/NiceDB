use std::ops::Range;

use nd_sql::Dialect;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Relational,
    Document,
    KeyValue,
    WideColumn,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Language {
    Sql { dialect: Dialect },
    Mql,
    Redis,
    Cql,
    Partiql,
    Search,
}

impl Language {
    pub fn dialect(self) -> Option<Dialect> {
        match self {
            Self::Sql { dialect } => Some(dialect),
            Self::Mql | Self::Redis | Self::Cql | Self::Partiql | Self::Search => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SchemaKind {
    Declared,
    Inferred,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ResultShape {
    Rows,
    Documents,
    Keys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceLevel {
    Always,
    Multiple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Tables,
    Views,
    Functions,
    Sequences,
    Types,
    Triggers,
    MaterializedViews,
    Dictionaries,
    Packages,
    Synonyms,
    Aliases,
    DataStreams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Cancel {
    Server,
    BetweenFetches,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TreeShape {
    pub databases: bool,
    pub namespaces: NamespaceLevel,
    pub groups: Vec<Group>,
    pub server: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Noun {
    Table,
    Collection,
    Key,
    Index,
    Column,
    Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct Vocabulary {
    pub table: Noun,
    pub column: Noun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Activity,
    Monitor,
    Space,
    Users,
    Compare,
    Rows,
    Backup,
    Transfer,
    Seclab,
    Search,
    Generate,
    Server,
    Cluster,
}

impl Tool {
    pub const ALL: [Self; 11] = [
        Self::Activity,
        Self::Monitor,
        Self::Space,
        Self::Users,
        Self::Compare,
        Self::Rows,
        Self::Backup,
        Self::Transfer,
        Self::Seclab,
        Self::Search,
        Self::Generate,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[allow(clippy::struct_excessive_bools)]
pub struct Capabilities {
    pub family: Family,
    pub language: Language,
    pub schema: SchemaKind,
    pub results: ResultShape,
    pub tree: TreeShape,
    pub words: Vocabulary,
    pub transactions: bool,
    pub editable: bool,
    pub explain: bool,
    pub er: bool,
    pub ddl: bool,
    pub sort: bool,
    pub params: bool,
    pub tools: Vec<Tool>,
    pub cancel: Cancel,
}

impl Capabilities {
    pub fn relational(dialect: Dialect) -> Self {
        Self {
            family: Family::Relational,
            language: Language::Sql { dialect },
            schema: SchemaKind::Declared,
            results: ResultShape::Rows,
            tree: TreeShape {
                databases: true,
                namespaces: NamespaceLevel::Always,
                groups: vec![
                    Group::Tables,
                    Group::Views,
                    Group::Functions,
                    Group::Sequences,
                    Group::Types,
                    Group::Triggers,
                ],
                server: true,
            },
            words: Vocabulary {
                table: Noun::Table,
                column: Noun::Column,
            },
            transactions: true,
            editable: true,
            explain: true,
            er: true,
            ddl: true,
            sort: true,
            params: true,
            tools: Tool::ALL.to_vec(),
            cancel: Cancel::Server,
        }
    }

    #[must_use]
    pub fn embedded(mut self) -> Self {
        self.tree.databases = false;
        self.tree.namespaces = NamespaceLevel::Multiple;
        self.tree.server = false;
        self.tools
            .retain(|t| !matches!(t, Tool::Activity | Tool::Users));
        self
    }

    #[must_use]
    pub fn groups(mut self, groups: &[Group]) -> Self {
        self.tree.groups = groups.to_vec();
        self
    }

    #[must_use]
    pub fn namespaces(mut self, level: NamespaceLevel) -> Self {
        self.tree.namespaces = level;
        self
    }

    pub fn dialect(&self) -> Option<Dialect> {
        self.language.dialect()
    }

    pub fn has(&self, tool: Tool) -> bool {
        self.tools.contains(&tool)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandClass {
    pub write: bool,
    pub risk: Option<nd_sql::Risk>,
}

pub type Commands = Vec<Range<usize>>;
