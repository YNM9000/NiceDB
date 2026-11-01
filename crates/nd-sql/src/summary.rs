use serde::{Deserialize, Serialize};

use crate::dml::{Bound, Browse, Builder, filters};
use crate::sort::{Unsortable, single};
use crate::{Dialect, ident};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SummaryColumn {
    pub name: String,
    pub numeric: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum GroupFn {
    Sum,
    Count,
    Avg,
    Min,
    Max,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct GroupValue {
    pub column: Option<String>,
    pub func: GroupFn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Summary {
    Stats {
        columns: Vec<SummaryColumn>,
    },
    Group {
        x: String,
        values: Vec<GroupValue>,
        limit: u32,
    },
}

pub fn summarizable(dialect: Dialect) -> bool {
    !matches!(
        dialect,
        Dialect::Cql | Dialect::Partiql | Dialect::Opensearch | Dialect::Elasticsearch
    )
}

fn as_text(dialect: Dialect, expr: &str) -> String {
    match dialect {
        Dialect::Postgres => format!("({expr})::text"),
        Dialect::Clickhouse => format!("toString({expr})"),
        _ => expr.to_owned(),
    }
}

fn select_list(dialect: Dialect, summary: &Summary) -> String {
    let q = |c: &str| ident(dialect, c);
    let t = |e: String| as_text(dialect, &e);
    let mut out = Vec::new();
    match summary {
        Summary::Stats { columns } => {
            out.push(t("COUNT(*)".into()));
            for c in columns {
                let n = q(&c.name);
                out.push(t(format!("COUNT({n})")));
                if c.numeric {
                    out.push(t(format!("SUM({n})")));
                    out.push(t(format!("MIN({n})")));
                    out.push(t(format!("MAX({n})")));
                } else {
                    out.push(t(format!("COUNT(DISTINCT {n})")));
                }
            }
        }
        Summary::Group { x, values, .. } => {
            out.push(format!("{} AS {}", t(q(x)), q("x")));
            for (i, v) in values.iter().enumerate() {
                let arg = match v.column.as_deref() {
                    None => "*".to_owned(),
                    Some(c)
                        if v.func == GroupFn::Avg
                            && matches!(dialect, Dialect::Mssql | Dialect::Firebird) =>
                    {
                        format!("CAST({} AS DOUBLE PRECISION)", q(c))
                    }
                    Some(c) => q(c),
                };
                let f = match v.func {
                    GroupFn::Sum => "SUM",
                    GroupFn::Count => "COUNT",
                    GroupFn::Avg => "AVG",
                    GroupFn::Min => "MIN",
                    GroupFn::Max => "MAX",
                };
                out.push(format!(
                    "{} AS {}",
                    t(format!("{f}({arg})")),
                    q(&format!("v{i}"))
                ));
            }
        }
    }
    out.join(", ")
}

fn tail(dialect: Dialect, summary: &Summary, x_ref: &str) -> String {
    let Summary::Group { limit, .. } = summary else {
        return String::new();
    };
    let limit = match dialect {
        Dialect::Mssql | Dialect::Firebird | Dialect::Hana => {
            format!(" ORDER BY 1 OFFSET 0 ROWS FETCH NEXT {limit} ROWS ONLY")
        }
        _ => format!(" LIMIT {limit}"),
    };
    format!(" GROUP BY {x_ref}{limit}")
}

fn group_ref(dialect: Dialect, summary: &Summary) -> String {
    match summary {
        Summary::Group { x, .. } => ident(dialect, x),
        Summary::Stats { .. } => String::new(),
    }
}

pub fn summary_query(sql: &str, dialect: Dialect, summary: &Summary) -> Result<String, Unsortable> {
    let text = single(sql, dialect)?;
    if !summarizable(dialect) {
        return Err(Unsortable::NotSelect);
    }
    Ok(format!(
        "SELECT {} FROM (\n{text}\n) AS q{}",
        select_list(dialect, summary),
        tail(dialect, summary, &group_ref(dialect, summary))
    ))
}

pub fn summary_browse(dialect: Dialect, browse: &Browse, summary: &Summary) -> Bound {
    let mut q = Builder::new(dialect);
    q.text("SELECT ")
        .text(&select_list(dialect, summary))
        .text(" FROM ")
        .table(&browse.table);
    filters(&mut q, browse, None);
    let x_ref = match summary {
        Summary::Group { x, .. } => format!(
            "{}.{}",
            ident(dialect, &browse.table.name),
            ident(dialect, x)
        ),
        Summary::Stats { .. } => String::new(),
    };
    q.text(&tail(dialect, summary, &x_ref));
    q.b
}
