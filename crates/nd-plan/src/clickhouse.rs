use std::fmt::Write as _;

use serde_json::{Map, Value};

use crate::model::{Engine, MAX_DEPTH, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

const GRANULE: f64 = 8192.0;

fn kind(op: &str) -> NodeKind {
    match op {
        "ReadFromMergeTree" | "ReadFromStorage" | "ReadFromSystemNumbers" | "ReadFromRemote" => {
            NodeKind::FullScan
        }
        "Aggregating" | "MergingAggregated" | "TotalsHaving" => NodeKind::Aggregate,
        "Sorting" | "MergingSorted" | "FinishSorting" | "PartialSorting" => NodeKind::Sort,
        "Limit" | "LimitBy" | "Offset" => NodeKind::Limit,
        "Join" | "JoiningTransform" | "FilledJoin" => NodeKind::HashJoin,
        "Union" => NodeKind::Append,
        "CreatingSets" | "CreatingSet" => NodeKind::Subquery,
        "Window" => NodeKind::Window,
        "Distinct" | "PreDistinct" => NodeKind::Unique,
        _ => NodeKind::Other,
    }
}

fn count(o: &Map<String, Value>, key: &str) -> Option<f64> {
    o.get(key).and_then(Value::as_f64)
}

fn read(node: &mut Node, indexes: &[Value]) {
    let mut last = None;
    let mut cut = false;
    for ix in indexes.iter().filter_map(Value::as_object) {
        let ty = ix.get("Type").and_then(Value::as_str).unwrap_or("Index");
        let name = ix.get("Name").and_then(Value::as_str);
        let keys: Vec<&str> = ix
            .get("Keys")
            .and_then(Value::as_array)
            .map(|k| k.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let (parts, of_parts) = (count(ix, "Selected Parts"), count(ix, "Initial Parts"));
        let (granules, of_granules) = (
            count(ix, "Selected Granules"),
            count(ix, "Initial Granules"),
        );
        let mut value = String::new();
        if !keys.is_empty() {
            let _ = write!(value, "({}) ", keys.join(", "));
        }
        if let (Some(p), Some(op)) = (parts, of_parts) {
            let _ = write!(value, "parts {p}/{op}");
        }
        if let (Some(g), Some(og)) = (granules, of_granules) {
            let _ = write!(value, " · granules {g}/{og}");
            if g < og && ty != "MinMax" {
                cut = true;
            }
            last = Some(g);
        }
        node.detail(
            &name.map_or_else(|| ty.to_owned(), |n| format!("{ty} {n}")),
            value.trim(),
        );
        if let Some(c) = ix.get("Condition").and_then(Value::as_str)
            && c != "true"
        {
            node.condition.get_or_insert_with(|| c.to_owned());
        }
        if ty == "PrimaryKey" {
            node.index = Some(keys.join(", "));
        }
    }
    if cut {
        node.kind = NodeKind::IndexScan;
    }
    node.est_rows = last.map(|g| g * GRANULE);
}

fn tree(o: &Map<String, Value>, depth: u32) -> Tree {
    let op = o.get("Node Type").and_then(Value::as_str).unwrap_or("?");
    let mut node = Node::new(op, kind(op));
    let description = o.get("Description").and_then(Value::as_str).unwrap_or("");
    if node.kind.is_scan() {
        let table = description.rsplit('.').next().unwrap_or(description);
        node.relation = (!table.is_empty()).then(|| table.to_owned());
        if let Some(ix) = o.get("Indexes").and_then(Value::as_array) {
            read(&mut node, ix);
        }
    } else if op == "Filter" {
        node.filter = (!description.is_empty()).then(|| description.to_owned());
    } else {
        node.detail("description", description);
    }
    let children = if depth < MAX_DEPTH {
        o.get("Plans")
            .and_then(Value::as_array)
            .map(|p| {
                p.iter()
                    .filter_map(Value::as_object)
                    .map(|c| tree(c, depth + 1))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    collapse(Tree { node, children })
}

fn collapse(mut t: Tree) -> Tree {
    if t.node.op == "Expression"
        && t.children.len() == 1
        && let Some(child) = t.children.pop()
    {
        return child;
    }
    t
}

pub fn parse(raw: &str) -> Result<Plan> {
    let v: Value = serde_json::from_str(raw.trim()).map_err(|e| PlanError::Json(e.to_string()))?;
    let top = match &v {
        Value::Array(a) => a.first().cloned().unwrap_or(Value::Null),
        other => other.clone(),
    };
    let plan = top
        .get("Plan")
        .and_then(Value::as_object)
        .ok_or(PlanError::Shape("no Plan in the explain"))?;
    Ok(Plan::build(Engine::Clickhouse, false, tree(plan, 0), raw))
}
