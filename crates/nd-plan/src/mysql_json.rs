use serde_json::{Map, Value};

use crate::model::{Engine, MAX_DEPTH, Node, NodeKind, Plan, Tree};
use crate::mysql_tree::{classify, fold};
use crate::{PlanError, Result};

type Obj = Map<String, Value>;

fn num(o: &Obj, key: &str) -> Option<f64> {
    match o.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches(['K', 'M', 'G']).parse().ok(),
        _ => None,
    }
    .filter(|f: &f64| f.is_finite())
}

fn text(o: &Obj, key: &str) -> Option<String> {
    match o.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Array(a) => Some(
            a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
        )
        .filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn flag(o: &Obj, key: &str) -> bool {
    o.get(key) == Some(&Value::Bool(true))
}

fn obj<'a>(o: &'a Obj, key: &str) -> Option<&'a Obj> {
    o.get(key).and_then(Value::as_object)
}

fn access(kind: &str) -> NodeKind {
    match kind {
        "ALL" | "index" => NodeKind::FullScan,
        "const" | "system" | "eq_ref" | "ref" | "range" | "ref_or_null" | "fulltext"
        | "index_merge" | "unique_subquery" | "index_subquery" | "hash" => NodeKind::IndexScan,
        _ => NodeKind::Other,
    }
}

fn table(t: &Obj, depth: u32) -> Tree {
    let access_type = text(t, "access_type").unwrap_or_default();
    let mut kind = access(&access_type);
    if kind == NodeKind::IndexScan && flag(t, "using_index") {
        kind = NodeKind::IndexOnlyScan;
    }
    let name = text(t, "table_name").unwrap_or_else(|| "?".to_owned());
    let mut n = Node::new(format!("{access_type} {name}").trim().to_owned(), kind);
    if !name.starts_with('<') {
        n.relation = Some(name);
    }
    n.index = text(t, "key");
    n.condition =
        text(t, "ref").map(|r| format!("{} = {r}", text(t, "used_key_parts").unwrap_or_default()));
    n.filter = text(t, "attached_condition");
    let examined = num(t, "rows_examined_per_scan").or_else(|| num(t, "rows"));
    let filtered = num(t, "filtered").unwrap_or(100.0) / 100.0;
    n.est_examined = examined;
    n.est_rows = examined.map(|e| e * filtered);
    n.total_cost = obj(t, "cost_info")
        .and_then(|c| num(c, "prefix_cost").or_else(|| num(c, "read_cost")))
        .or_else(|| num(t, "cost"));
    if let Some(loops) = num(t, "r_loops") {
        let read = num(t, "r_rows").unwrap_or(0.0);
        let kept = read * num(t, "r_filtered").unwrap_or(100.0) / 100.0;
        n.loops = Some(loops);
        n.rows = Some(kept);
        n.rows_removed = Some((read - kept).max(0.0));
        let time =
            num(t, "r_table_time_ms").unwrap_or(0.0) + num(t, "r_other_time_ms").unwrap_or(0.0);
        n.time_ms = Some(num(t, "r_total_time_ms").unwrap_or(time));
    }
    for key in [
        "possible_keys",
        "used_columns",
        "key_length",
        "rows_produced_per_join",
        "filtered",
    ] {
        if let Some(v) = text(t, key).or_else(|| num(t, key).map(|f| f.to_string())) {
            n.detail(key, v);
        }
    }
    let mut children = Vec::new();
    if depth < MAX_DEPTH {
        for key in ["materialized_from_subquery", "attached_subqueries"] {
            children.extend(blocks(t.get(key), depth + 1));
        }
    }
    Tree { node: n, children }
}

fn blocks(v: Option<&Value>, depth: u32) -> Vec<Tree> {
    let one = |o: &Obj| obj(o, "query_block").map_or_else(|| block(o, depth), |b| block(b, depth));
    match v {
        Some(Value::Object(o)) => one(o).into_iter().collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_object)
            .filter_map(one)
            .collect(),
        _ => Vec::new(),
    }
}

fn inner(o: &Obj, depth: u32) -> Option<Tree> {
    if depth >= MAX_DEPTH {
        return None;
    }
    if let Some(Value::Array(list)) = o.get("nested_loop") {
        let mut steps = list
            .iter()
            .filter_map(Value::as_object)
            .filter_map(|s| step(s, depth + 1));
        let first = steps.next()?;
        return Some(steps.fold(first, |acc, next| {
            let mut join = Node::new("Nested loop", NodeKind::NestedLoop);
            join.est_rows = next
                .node
                .est_rows
                .zip(acc.node.est_rows)
                .map(|(a, b)| a * b);
            Tree {
                node: join,
                children: vec![acc, next],
            }
        }));
    }
    if let Some(t) = obj(o, "table") {
        return Some(table(t, depth + 1));
    }
    for (key, op, kind) in [
        ("ordering_operation", "Sort", NodeKind::Sort),
        ("grouping_operation", "Group", NodeKind::Aggregate),
        ("duplicates_removal", "Remove duplicates", NodeKind::Unique),
        ("windowing", "Window", NodeKind::Window),
        ("filesort", "Filesort", NodeKind::Sort),
        ("read_sorted_file", "Read sorted file", NodeKind::Sort),
        ("temporary_table", "Temporary table", NodeKind::Materialize),
        ("having", "Having", NodeKind::Other),
    ] {
        if let Some(sub) = obj(o, key) {
            let child = inner(sub, depth + 1);
            let skip = !flag(sub, "using_filesort")
                && !flag(sub, "using_temporary_table")
                && matches!(key, "ordering_operation" | "grouping_operation");
            if skip
                || (key == "read_sorted_file"
                    && child
                        .as_ref()
                        .is_some_and(|c| c.node.kind == NodeKind::Sort))
            {
                if let Some(c) = child {
                    return Some(c);
                }
                continue;
            }
            let mut n = Node::new(op, kind);
            for k in [
                "sort_key",
                "r_sort_mode",
                "using_filesort",
                "using_temporary_table",
            ] {
                if let Some(v) = text(sub, k).or_else(|| {
                    sub.get(k)
                        .filter(|v| v.is_boolean())
                        .map(ToString::to_string)
                }) {
                    n.detail(k, v);
                }
            }
            if let Some(loops) = num(sub, "r_loops") {
                n.loops = Some(loops);
                n.rows = num(sub, "r_output_rows").or_else(|| num(sub, "r_rows"));
                n.time_ms = num(sub, "r_total_time_ms");
            }
            return Some(Tree {
                node: n,
                children: child.into_iter().collect(),
            });
        }
    }
    if let Some(u) = obj(o, "union_result") {
        let mut n = Node::new("Union", NodeKind::Append);
        n.relation = None;
        let children = blocks(u.get("query_specifications"), depth + 1);
        return Some(Tree { node: n, children });
    }
    None
}

fn step(s: &Obj, depth: u32) -> Option<Tree> {
    if let Some(t) = obj(s, "table") {
        return Some(table(t, depth));
    }
    let bnl = obj(s, "block-nl-join").or_else(|| obj(s, "bka-join"))?;
    let mut tree = obj(bnl, "table").map(|t| table(t, depth))?;
    if let Some(cond) = text(bnl, "attached_condition") {
        tree.node.filter = Some(cond);
    }
    if let Some(f) = num(bnl, "r_filtered")
        && let Some(rows) = tree.node.rows
    {
        tree.node.rows = Some(rows * f / 100.0);
    }
    Some(tree)
}

fn block(b: &Obj, depth: u32) -> Option<Tree> {
    let mut tree = inner(b, depth)?;
    let subs = blocks(b.get("subqueries"), depth + 1);
    if !subs.is_empty() {
        tree.children.extend(subs);
    }
    if tree.node.time_ms.is_none()
        && let Some(t) = num(b, "r_total_time_ms")
    {
        tree.node.time_ms = Some(t);
    }
    Some(tree)
}

fn operation(o: &Obj, depth: u32) -> Tree {
    let mut n = classify(&text(o, "operation").unwrap_or_else(|| "?".to_owned()));
    if let Some(t) = text(o, "table_name") {
        n.relation = Some(text(o, "alias").unwrap_or(t));
    }
    if let Some(i) = text(o, "index_name") {
        n.index = Some(i);
    }
    if let Some(c) = text(o, "condition") {
        if n.op == "Filter" {
            n.filter = Some(c);
        } else {
            n.condition = Some(c);
        }
    }
    n.est_rows = num(o, "estimated_rows");
    n.total_cost = num(o, "estimated_total_cost");
    if let Some(loops) = num(o, "actual_loops") {
        n.loops = Some(loops);
        n.rows = num(o, "actual_rows");
        n.time_ms = num(o, "actual_last_row_ms").map(|t| t * loops);
    }
    let children = match o.get("inputs") {
        Some(Value::Array(a)) if depth < MAX_DEPTH => a
            .iter()
            .filter_map(Value::as_object)
            .map(|c| operation(c, depth + 1))
            .collect(),
        _ => Vec::new(),
    };
    Tree { node: n, children }
}

pub fn parse(raw: &str, engine: Engine) -> Result<Plan> {
    let v: Value = serde_json::from_str(raw.trim()).map_err(|e| PlanError::Json(e.to_string()))?;
    let top = v
        .as_object()
        .ok_or(PlanError::Shape("expected an object"))?;
    let tree = if top.contains_key("operation") {
        fold(operation(top, 0))
    } else {
        let qb = obj(top, "query_block").ok_or(PlanError::Shape("missing \"query_block\""))?;
        block(qb, 0).ok_or(PlanError::Shape("empty query block"))?
    };
    let analyzed = raw.contains("\"r_loops\"") || raw.contains("\"actual_loops\"");
    let mut plan = Plan::build(engine, analyzed, tree, raw);
    if let Some(t) = obj(top, "query_block").and_then(|b| num(b, "r_total_time_ms")) {
        plan.total_ms = Some(t);
    }
    if let Some(t) = obj(top, "query_optimization").and_then(|b| num(b, "r_total_time_ms")) {
        plan.planning_ms = Some(t);
    }
    Ok(plan)
}
