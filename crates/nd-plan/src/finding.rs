use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    SeqScan,
    FkWithoutIndex,
    RowEstimate,
    Spill,
    NestedLoop,
    FunctionOnIndexedColumn,
    ImplicitCast,
    SelectStarSort,
    MissingLimit,
    DocsExamined,
    InMemorySort,
}

impl Rule {
    pub fn slug(self) -> &'static str {
        match self {
            Self::SeqScan => "seq_scan",
            Self::FkWithoutIndex => "fk_without_index",
            Self::RowEstimate => "row_estimate",
            Self::Spill => "spill",
            Self::NestedLoop => "nested_loop",
            Self::FunctionOnIndexedColumn => "function_on_indexed_column",
            Self::ImplicitCast => "implicit_cast",
            Self::SelectStarSort => "select_star_sort",
            Self::MissingLimit => "missing_limit",
            Self::DocsExamined => "docs_examined",
            Self::InMemorySort => "in_memory_sort",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Finding {
    pub rule: Rule,
    pub severity: Severity,
    pub node: Option<u32>,
    pub table: Option<String>,
    pub column: Option<String>,
    pub wrapper: Option<String>,
    pub rows: Option<f64>,
    pub kept: Option<f64>,
    pub factor: Option<f64>,
    pub share: Option<f64>,
    pub kb: Option<f64>,
    pub suggestion: Option<String>,
}

impl Finding {
    pub fn new(rule: Rule, severity: Severity, node: Option<u32>) -> Self {
        Self {
            rule,
            severity,
            node,
            table: None,
            column: None,
            wrapper: None,
            rows: None,
            kept: None,
            factor: None,
            share: None,
            kb: None,
            suggestion: None,
        }
    }
}
