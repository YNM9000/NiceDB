use serde_json::{Map, Value};

use crate::model::{Engine, MAX_DEPTH, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

const WRAPPERS: &[&str] = &[
    "RESULT_COLLECTOR",
    "EXECUTE",
    "EXPLAIN_ANALYZE",
    "EXPLAIN",
    "QUERY",
];

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_owned()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(text).collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

fn num(o: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| match o.get(*k)? {
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        Value::String(s) => s
            .trim()
            .trim_start_matches('~')
            .parse()
            .ok()
            .filter(|f: &f64| f.is_finite()),
        _ => None,
    })
}

fn name(o: &Map<String, Value>) -> String {
    ["type", "operator_type", "name", "operator_name"]
        .iter()
        .find_map(|k| o.get(*k).and_then(text))
        .unwrap_or_else(|| "?".to_owned())
}

fn kind(op: &str, scan_type: Option<&str>) -> NodeKind {
    match op {
        "TABLE_SCAN" | "SEQ_SCAN" | "READ_PARQUET" | "READ_CSV" | "READ_CSV_AUTO" | "READ_JSON" => {
            if scan_type.is_some_and(|t| t.contains("Index")) {
                NodeKind::IndexScan
            } else {
                NodeKind::FullScan
            }
        }
        "INDEX_SCAN" => NodeKind::IndexScan,
        "HASH_JOIN" | "DELIM_JOIN" | "LEFT_DELIM_JOIN" | "RIGHT_DELIM_JOIN" | "ASOF_JOIN"
        | "IE_JOIN" | "RANGE_JOIN" => NodeKind::HashJoin,
        "NESTED_LOOP_JOIN" | "BLOCKWISE_NL_JOIN" | "CROSS_PRODUCT" | "POSITIONAL_JOIN" => {
            NodeKind::NestedLoop
        }
        "PIECEWISE_MERGE_JOIN" => NodeKind::MergeJoin,
        "HASH_GROUP_BY"
        | "PERFECT_HASH_GROUP_BY"
        | "UNGROUPED_AGGREGATE"
        | "SIMPLE_AGGREGATE"
        | "PARTITIONED_AGGREGATE"
        | "STREAMING_GROUP_BY" => NodeKind::Aggregate,
        "ORDER_BY" | "TOP_N" => NodeKind::Sort,
        "LIMIT" | "STREAMING_LIMIT" | "LIMIT_PERCENT" => NodeKind::Limit,
        "WINDOW" | "STREAMING_WINDOW" => NodeKind::Window,
        "UNION" | "RECURSIVE_CTE" => NodeKind::Append,
        "CTE" | "CTE_SCAN" | "DELIM_SCAN" | "COLUMN_DATA_SCAN" | "CHUNK_SCAN"
        | "TABLE_FUNCTION" => NodeKind::Subquery,
        "PROJECTION" | "RESULT_COLLECTOR" | "DUMMY_SCAN" | "EMPTY_RESULT" => NodeKind::Result,
        "INSERT" | "UPDATE" | "DELETE" | "BATCH_INSERT" | "BATCH_COPY_TO_FILE" | "COPY_TO_FILE" => {
            NodeKind::Modify
        }
        "HASH_DISTINCT" => NodeKind::Unique,
        _ => NodeKind::Other,
    }
}

fn node(o: &Map<String, Value>, depth: u32) -> Tree {
    let op = name(o);
    let empty = Map::new();
    let extra = o
        .get("extra_info")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let scan_type = extra.get("Type").and_then(text);
    let kind = kind(op.trim(), scan_type.as_deref());
    let label = match &scan_type {
        Some(t) if kind.is_scan() => t.clone(),
        _ => op.trim().to_owned(),
    };
    let mut n = Node::new(label, kind);
    n.relation = extra
        .get("Table")
        .and_then(text)
        .map(|t| t.rsplit('.').next().unwrap_or(&t).to_owned());
    n.index = extra.get("Index").and_then(text);
    n.filter = extra
        .get("Filters")
        .or_else(|| extra.get("Expression"))
        .and_then(|f| match f {
            Value::Array(a) => {
                let parts: Vec<String> = a.iter().filter_map(text).collect();
                (!parts.is_empty()).then(|| parts.join(" AND "))
            }
            other => text(other),
        })
        .map(|f| f.replace('\n', " AND "));
    n.condition = extra.get("Conditions").and_then(text);
    n.est_rows = num(extra, &["Estimated Cardinality", "Estimated Cardinality "]);
    let measured = num(o, &["intermediate_rows", "operator_cardinality"]);
    if let Some(rows) = measured {
        n.rows = Some(rows);
        n.loops = Some(1.0);
        if let Some(scanned) =
            num(o, &["rows_scanned", "operator_rows_scanned"]).filter(|s| *s > 0.0)
        {
            n.rows_removed = Some((scanned - rows).max(0.0));
        }
        n.time_ms = num(o, &["timing", "operator_timing"]).map(|s| s * 1_000.0);
    }
    for (k, v) in extra {
        if matches!(
            k.as_str(),
            "Table" | "Type" | "Filters" | "Conditions" | "Estimated Cardinality" | "Index"
        ) {
            continue;
        }
        if let Some(v) = text(v) {
            n.detail(k, v);
        }
    }
    if let Some(groups) = num(o, &["row_groups_scanned"]) {
        let of = num(o, &["total_row_groups_to_scan"]).map_or(String::new(), |t| format!(" / {t}"));
        n.detail("row groups", format!("{groups}{of}"));
    }
    let children = if depth >= MAX_DEPTH {
        Vec::new()
    } else {
        o.get("children")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_object)
                    .map(|c| node(c, depth + 1))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut t = Tree { node: n, children };
    if t.node.time_ms.is_some() {
        let below: f64 = t.children.iter().filter_map(|c| c.node.time_ms).sum();
        t.node.time_ms = t.node.time_ms.map(|own| own + below);
    }
    t
}

fn unwrap(mut t: Tree) -> Tree {
    while t.children.len() == 1 && WRAPPERS.contains(&t.node.op.as_str()) {
        let Some(child) = t.children.pop() else { break };
        t = child;
    }
    t
}

fn fold_filters(mut t: Tree) -> Tree {
    t.children = t.children.into_iter().map(fold_filters).collect();
    let foldable = t.node.op == "FILTER"
        && t.children.len() == 1
        && t.children[0].node.kind.is_scan()
        && t.children[0].node.filter.is_none();
    if !foldable {
        return t;
    }
    let Some(mut scan) = t.children.pop() else {
        return t;
    };
    let f = t.node;
    let s = &mut scan.node;
    s.filter = f.filter;
    s.est_examined = s.est_rows;
    s.est_rows = f.est_rows.or(s.est_rows);
    if let (Some(read), Some(kept)) = (s.rows, f.rows) {
        s.rows_removed = Some((read - kept).max(0.0) + s.rows_removed.unwrap_or(0.0));
        s.rows = Some(kept);
    }
    s.time_ms = f.time_ms.or(s.time_ms);
    scan
}

pub fn parse(raw: &str) -> Result<Plan> {
    let v: Value = serde_json::from_str(raw.trim()).map_err(|e| PlanError::Json(e.to_string()))?;
    let (tops, profile) = match &v {
        Value::Array(a) => (a.clone(), None),
        Value::Object(o) => {
            let tops = o
                .get("operator")
                .or_else(|| o.get("children"))
                .and_then(Value::as_array)
                .cloned()
                .ok_or(PlanError::Shape("no operators in the profile"))?;
            (tops, Some(o))
        }
        _ => return Err(PlanError::Shape("expected an array or an object")),
    };
    let mut roots: Vec<Tree> = tops
        .iter()
        .filter_map(Value::as_object)
        .map(|o| fold_filters(unwrap(node(o, 1))))
        .collect();
    let root = match roots.len() {
        0 => return Err(PlanError::Shape("no operators")),
        1 => roots.remove(0),
        _ => Tree {
            node: Node::new("PLAN", NodeKind::Result),
            children: roots,
        },
    };
    let analyzed = profile.is_some();
    let mut plan = Plan::build(Engine::Duckdb, analyzed, root, raw);
    if let Some(o) = profile {
        let query = o.get("query").and_then(Value::as_object);
        let total = query
            .and_then(|q| num(q, &["total_time"]))
            .or_else(|| num(o, &["latency", "operator_timing"]));
        if let Some(total) = total {
            plan.total_ms = Some(total * 1_000.0);
        }
        plan.planning_ms = o
            .get("planner")
            .and_then(Value::as_object)
            .and_then(|p| num(p, &["total_time"]))
            .map(|s| s * 1_000.0);
    }
    Ok(plan)
}
