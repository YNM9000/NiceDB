use serde_json::{Map, Value};

use crate::model::{Buffers, Engine, MAX_DEPTH, Node, NodeKind, Plan, Spill, SpillKind, Tree};
use crate::{PlanError, Result};

const KNOWN: &[&str] = &[
    "Node Type",
    "Plans",
    "Relation Name",
    "Alias",
    "Index Name",
    "Hash Cond",
    "Merge Cond",
    "Index Cond",
    "Recheck Cond",
    "Filter",
    "Join Filter",
    "Rows Removed by Filter",
    "Startup Cost",
    "Total Cost",
    "Plan Rows",
    "Plan Width",
    "Actual Startup Time",
    "Actual Total Time",
    "Actual Rows",
    "Actual Loops",
    "Parallel Aware",
    "Async Capable",
    "Workers",
    "Partial Mode",
    "Strategy",
    "Join Type",
];

fn num(o: &Map<String, Value>, key: &str) -> Option<f64> {
    match o.get(key)? {
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        Value::String(s) => s.trim().parse().ok().filter(|f: &f64| f.is_finite()),
        _ => None,
    }
}

fn text(o: &Map<String, Value>, key: &str) -> Option<String> {
    match o.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn flat(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(a) if a.iter().all(|x| !x.is_object() && !x.is_array()) => {
            Some(a.iter().filter_map(flat).collect::<Vec<_>>().join(", "))
        }
        _ => None,
    }
}

fn kind(node_type: &str) -> NodeKind {
    match node_type {
        "Seq Scan" | "YB Seq Scan" | "Sample Scan" | "Tid Scan" | "Tid Range Scan"
        | "Foreign Scan" => NodeKind::FullScan,
        "Index Scan" => NodeKind::IndexScan,
        "Index Only Scan" => NodeKind::IndexOnlyScan,
        "Bitmap Heap Scan" | "YB Bitmap Table Scan" => NodeKind::BitmapScan,
        "Bitmap Index Scan" | "BitmapAnd" | "BitmapOr" => NodeKind::BitmapIndex,
        "Hash Join" => NodeKind::HashJoin,
        "Nested Loop" | "YB Batched Nested Loop" => NodeKind::NestedLoop,
        "Merge Join" => NodeKind::MergeJoin,
        "Hash" => NodeKind::Hash,
        "Aggregate" | "Group" => NodeKind::Aggregate,
        "Sort" | "Incremental Sort" => NodeKind::Sort,
        "Limit" => NodeKind::Limit,
        "Gather" | "Gather Merge" => NodeKind::Gather,
        "Materialize" | "Memoize" => NodeKind::Materialize,
        "Subquery Scan"
        | "CTE Scan"
        | "Function Scan"
        | "Values Scan"
        | "WorkTable Scan"
        | "Table Function Scan"
        | "Named Tuplestore Scan" => NodeKind::Subquery,
        "Append" | "Merge Append" | "Recursive Union" | "SetOp" => NodeKind::Append,
        "WindowAgg" => NodeKind::Window,
        "Unique" => NodeKind::Unique,
        "ModifyTable" => NodeKind::Modify,
        "Result" | "ProjectSet" => NodeKind::Result,
        _ => NodeKind::Other,
    }
}

fn op_name(o: &Map<String, Value>, node_type: &str) -> String {
    let mut name = match (node_type, text(o, "Strategy").as_deref()) {
        ("Aggregate", Some("Hashed")) => "HashAggregate".to_owned(),
        ("Aggregate", Some("Sorted")) => "GroupAggregate".to_owned(),
        ("Aggregate", Some("Mixed")) => "MixedAggregate".to_owned(),
        ("ModifyTable", _) => text(o, "Operation").unwrap_or_else(|| node_type.to_owned()),
        _ => node_type.to_owned(),
    };
    if let Some(join) = text(o, "Join Type").filter(|j| j != "Inner") {
        name = if name == "Nested Loop" {
            format!("Nested Loop {join} Join")
        } else {
            name.replace("Join", &format!("{join} Join"))
        };
    }
    if let Some(mode) = text(o, "Partial Mode").filter(|m| m != "Simple") {
        name = format!("{mode} {name}");
    }
    if o.get("Parallel Aware") == Some(&Value::Bool(true)) {
        name = format!("Parallel {name}");
    }
    name
}

fn buffers(o: &Map<String, Value>) -> Option<Buffers> {
    o.get("Shared Hit Blocks")?;
    let n = |k| num(o, k).unwrap_or(0.0);
    Some(Buffers {
        hit: n("Shared Hit Blocks") + n("Local Hit Blocks"),
        read: n("Shared Read Blocks") + n("Local Read Blocks"),
        written: n("Shared Written Blocks") + n("Local Written Blocks"),
        temp_read: n("Temp Read Blocks"),
        temp_written: n("Temp Written Blocks"),
    })
}

fn spill(o: &Map<String, Value>, kind: NodeKind) -> Option<Spill> {
    if text(o, "Sort Space Type").as_deref() == Some("Disk") {
        return Some(Spill {
            kind: SpillKind::Sort,
            kb: num(o, "Sort Space Used"),
        });
    }
    if kind == NodeKind::Hash && num(o, "Hash Batches").is_some_and(|b| b > 1.0) {
        return Some(Spill {
            kind: SpillKind::Hash,
            kb: None,
        });
    }
    let disk = num(o, "Disk Usage").filter(|d| *d > 0.0);
    if kind == NodeKind::Aggregate && (disk.is_some() || num(o, "HashAgg Batches") > Some(1.0)) {
        return Some(Spill {
            kind: SpillKind::Aggregate,
            kb: disk,
        });
    }
    None
}

fn node(o: &Map<String, Value>, parallel: bool, depth: u32) -> Tree {
    let node_type = text(o, "Node Type").unwrap_or_else(|| "?".to_owned());
    let kind = kind(&node_type);
    let parallel = parallel || o.get("Parallel Aware") == Some(&Value::Bool(true));
    let mut n = Node::new(op_name(o, &node_type), kind);
    n.relation = text(o, "Relation Name")
        .or_else(|| text(o, "CTE Name"))
        .or_else(|| text(o, "Function Name"));
    n.alias = text(o, "Alias");
    n.index = text(o, "Index Name");
    n.condition = text(o, "Hash Cond")
        .or_else(|| text(o, "Merge Cond"))
        .or_else(|| text(o, "Index Cond"))
        .or_else(|| text(o, "Recheck Cond"));
    n.filter = text(o, "Filter").or_else(|| text(o, "Join Filter"));
    n.est_rows = num(o, "Plan Rows");
    n.width = num(o, "Plan Width");
    n.startup_cost = num(o, "Startup Cost");
    n.total_cost = num(o, "Total Cost");
    n.loops = num(o, "Actual Loops");
    n.parallel = parallel;
    if let Some(loops) = n.loops {
        n.rows = Some(num(o, "Actual Rows").unwrap_or(0.0));
        n.rows_removed = num(o, "Rows Removed by Filter");
        let per_loop = num(o, "Actual Total Time").unwrap_or(0.0);
        n.time_ms = Some(if parallel { per_loop } else { per_loop * loops });
    }
    n.buffers = buffers(o);
    n.spill = spill(o, kind);
    for (k, v) in o {
        if KNOWN.contains(&k.as_str()) || k.contains("Blocks") || k.contains("I/O") {
            continue;
        }
        if let Some(value) = flat(v) {
            n.detail(k, value);
        }
    }
    if o.contains_key("Filter") && o.contains_key("Join Filter") {
        n.detail("Join Filter", text(o, "Join Filter").unwrap_or_default());
    }

    let gathers = kind == NodeKind::Gather;
    let children = if depth >= MAX_DEPTH {
        Vec::new()
    } else {
        match o.get("Plans") {
            Some(Value::Array(plans)) => plans
                .iter()
                .filter_map(Value::as_object)
                .map(|c| node(c, parallel || gathers, depth + 1))
                .collect(),
            _ => Vec::new(),
        }
    };
    Tree { node: n, children }
}

pub fn parse(raw: &str) -> Result<Plan> {
    let v: Value = serde_json::from_str(raw.trim()).map_err(|e| PlanError::Json(e.to_string()))?;
    let top = match &v {
        Value::Array(a) => a.first().and_then(Value::as_object),
        Value::Object(o) => Some(o),
        _ => None,
    }
    .ok_or(PlanError::Shape("expected an array with one plan"))?;
    let root = match top.get("Plan") {
        Some(Value::Object(p)) => p,
        _ if top.contains_key("Node Type") => top,
        _ => return Err(PlanError::Shape("missing \"Plan\"")),
    };
    let tree = node(root, false, 0);
    let analyzed = tree.node.loops.is_some() || top.contains_key("Execution Time");
    let mut plan = Plan::build(Engine::Postgres, analyzed, tree, raw);
    plan.planning_ms = num(top, "Planning Time");
    if let Some(total) = num(top, "Execution Time") {
        plan.total_ms = Some(total);
    }
    Ok(plan)
}
