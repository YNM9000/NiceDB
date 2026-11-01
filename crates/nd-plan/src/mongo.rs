use serde_json::{Map, Value};

use crate::model::{Engine, MAX_DEPTH, Node, NodeKind, Plan, Spill, SpillKind, Tree};
use crate::{Catalog, Finding, PlanError, Result, Rule, Severity};

const BIG: f64 = 10_000.0;
const LOOSE: f64 = 10.0;
const LOOSE_MIN: f64 = 1_000.0;

fn kind(stage: &str) -> NodeKind {
    match stage {
        "COLLSCAN" => NodeKind::FullScan,
        "IXSCAN"
        | "IDHACK"
        | "EXPRESS_IXSCAN"
        | "EXPRESS_CLUSTERED_IXSCAN"
        | "CLUSTERED_IXSCAN"
        | "DISTINCT_SCAN"
        | "TEXT_MATCH" => NodeKind::IndexScan,
        "COUNT_SCAN" => NodeKind::IndexOnlyScan,
        "SORT" | "SORT_MERGE" | "$sort" => NodeKind::Sort,
        "LIMIT" | "SKIP" | "$limit" | "$skip" => NodeKind::Limit,
        "OR" | "$unionWith" => NodeKind::Append,
        "AND_HASH" | "AND_SORTED" => NodeKind::HashJoin,
        "GROUP" | "COUNT" | "$group" | "$count" | "$bucket" | "$bucketAuto" | "$sortByCount" => {
            NodeKind::Aggregate
        }
        "$lookup" | "EQ_LOOKUP" | "$graphLookup" => NodeKind::NestedLoop,
        "SHARD_MERGE" | "SHARDING_FILTER" | "SINGLE_SHARD" | "SHARD_MERGE_SORT" => NodeKind::Gather,
        "UPDATE" | "DELETE" | "BATCHED_DELETE" | "$out" | "$merge" => NodeKind::Modify,
        "$setWindowFields" => NodeKind::Window,
        "$facet" => NodeKind::Subquery,
        "EOF" => NodeKind::Result,
        _ => NodeKind::Other,
    }
}

fn num(o: &Map<String, Value>, key: &str) -> Option<f64> {
    o.get(key).and_then(Value::as_f64)
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn collection(namespace: &str) -> &str {
    namespace.split_once('.').map_or(namespace, |(_, c)| c)
}

fn stage(o: &Map<String, Value>, coll: Option<&str>, depth: u32) -> Tree {
    let name = o.get("stage").and_then(Value::as_str).unwrap_or("?");
    let mut node = Node::new(name, kind(name));
    node.rows = num(o, "nReturned");
    node.time_ms = num(o, "executionTimeMillisEstimate");
    let examined = num(o, "docsExamined").or_else(|| num(o, "keysExamined"));
    if matches!(
        node.kind,
        NodeKind::FullScan | NodeKind::IndexScan | NodeKind::IndexOnlyScan
    ) || name == "FETCH"
    {
        node.relation = coll.map(str::to_owned);
        if let (Some(read), Some(back)) = (examined, node.rows) {
            node.rows_removed = Some((read - back).max(0.0));
        }
    }
    if let Some(ix) = o.get("indexName").and_then(Value::as_str) {
        node.index = Some(ix.to_owned());
    }
    if let Some(bounds) = o.get("indexBounds") {
        node.condition = Some(compact(bounds));
    }
    if let Some(filter) = o.get("filter") {
        node.filter = Some(compact(filter));
    }
    for (key, label) in [
        ("keysExamined", "keysExamined"),
        ("docsExamined", "docsExamined"),
        ("direction", "direction"),
        ("isMultiKey", "isMultiKey"),
        ("limitAmount", "limitAmount"),
        ("totalDataSizeSorted", "totalDataSizeSorted"),
    ] {
        if let Some(v) = o.get(key) {
            node.detail(label, v.as_str().map_or_else(|| compact(v), str::to_owned));
        }
    }
    if let Some(p) = o.get("keyPattern") {
        node.detail("keyPattern", compact(p));
    }
    if let Some(p) = o.get("sortPattern") {
        node.detail("sortPattern", compact(p));
    }
    sorted_spill(&mut node, o, SpillKind::Sort);
    let mut children = Vec::new();
    if depth < MAX_DEPTH {
        for key in [
            "inputStage",
            "outerStage",
            "innerStage",
            "thenStage",
            "elseStage",
        ] {
            if let Some(child) = o.get(key).and_then(Value::as_object) {
                children.push(stage(child, coll, depth + 1));
            }
        }
        for child in o
            .get("inputStages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            children.push(stage(child, coll, depth + 1));
        }
    }
    Tree { node, children }
}

fn sorted_spill(node: &mut Node, o: &Map<String, Value>, kind: SpillKind) {
    if o.get("usedDisk").and_then(Value::as_bool) == Some(true) {
        node.spill = Some(Spill {
            kind,
            kb: num(o, "spilledDataStorageSize").map(|b| b / 1024.0),
        });
    }
}

fn query(o: &Map<String, Value>) -> Option<(Tree, bool)> {
    let planner = o.get("queryPlanner").and_then(Value::as_object)?;
    let coll = planner
        .get("namespace")
        .and_then(Value::as_str)
        .map(collection);
    let measured = o
        .get("executionStats")
        .and_then(Value::as_object)
        .and_then(|s| s.get("executionStages"))
        .and_then(Value::as_object);
    if let Some(stages) = measured {
        return Some((stage(stages, coll, 0), true));
    }
    let winning = planner.get("winningPlan").and_then(Value::as_object)?;
    let winning = winning
        .get("queryPlan")
        .and_then(Value::as_object)
        .unwrap_or(winning);
    Some((stage(winning, coll, 0), false))
}

fn pipeline(stages: &[Value]) -> Option<(Tree, bool)> {
    let mut below: Option<Tree> = None;
    let mut analyzed = false;
    for s in stages.iter().filter_map(Value::as_object) {
        let tree = if let Some(cursor) = s.get("$cursor").and_then(Value::as_object) {
            let (t, measured) = query(cursor)?;
            analyzed |= measured;
            t
        } else {
            let name = s
                .keys()
                .find(|k| k.starts_with('$'))
                .map_or("?", String::as_str);
            let mut node = Node::new(name, kind(name));
            node.rows = num(s, "nReturned");
            node.time_ms = num(s, "executionTimeMillisEstimate");
            if let Some(spec) = s.get(name) {
                node.detail("spec", compact(spec));
                if name == "$sort" {
                    if let Some(key) = spec.get("sortKey") {
                        node.detail("sortPattern", compact(key));
                    }
                    if let Some(bytes) = num(s, "totalDataSizeSortedBytesEstimate") {
                        node.detail("totalDataSizeSorted", bytes.to_string());
                    }
                }
                if name == "$lookup"
                    && let Some(from) = spec.get("from").and_then(Value::as_str)
                {
                    node.relation = Some(from.to_owned());
                }
            }
            let spill = if name == "$sort" {
                SpillKind::Sort
            } else {
                SpillKind::Aggregate
            };
            sorted_spill(&mut node, s, spill);
            Tree {
                node,
                children: below.take().into_iter().collect(),
            }
        };
        below = Some(tree);
    }
    below.map(|t| (t, analyzed))
}

pub(crate) fn parse(raw: &str) -> Result<Plan> {
    let v: Value = serde_json::from_str(raw).map_err(|e| PlanError::Json(e.to_string()))?;
    let o = v.as_object().ok_or(PlanError::Shape("not an object"))?;
    let (root, analyzed) = match o.get("stages").and_then(Value::as_array) {
        Some(stages) => pipeline(stages),
        None => query(o),
    }
    .ok_or(PlanError::Shape("no queryPlanner"))?;
    let mut plan = Plan::build(Engine::Mongodb, analyzed, root, raw);
    if let Some(stats) = o.get("executionStats").and_then(Value::as_object) {
        plan.total_ms = num(stats, "executionTimeMillis");
        if let Some(root) = plan.nodes.first_mut() {
            for key in ["nReturned", "totalKeysExamined", "totalDocsExamined"] {
                if let Some(n) = num(stats, key) {
                    root.detail(key, n.to_string());
                }
            }
        }
    }
    Ok(plan)
}

fn fields(filter: &Value, out: &mut Vec<(String, bool)>) {
    let Some(o) = filter.as_object() else { return };
    for (k, v) in o {
        if k == "$and" {
            for part in v.as_array().into_iter().flatten() {
                fields(part, out);
            }
        } else if !k.starts_with('$') && !out.iter().any(|(f, _)| f == k) {
            let equality = match v.as_object() {
                Some(ops) => ops.keys().all(|op| op == "$eq" || !op.starts_with('$')),
                None => true,
            };
            out.push((k.clone(), equality));
        }
    }
}

fn filter_fields(n: &Node) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    if let Some(f) = n
        .filter
        .as_deref()
        .and_then(|f| serde_json::from_str(f).ok())
    {
        fields(&f, &mut out);
    }
    out
}

fn sort_fields(n: &Node) -> Vec<(String, i64)> {
    n.details
        .iter()
        .find(|d| d.key == "sortPattern")
        .and_then(|d| serde_json::from_str::<Map<String, Value>>(&d.value).ok())
        .map(|m| {
            m.into_iter()
                .map(|(k, v)| (k, v.as_i64().unwrap_or(1)))
                .collect()
        })
        .unwrap_or_default()
}

fn create_index(collection: &str, keys: &[(String, i64)]) -> String {
    let mut seen = Vec::new();
    let key: Vec<String> = keys
        .iter()
        .filter(|(k, _)| {
            let fresh = !seen.contains(k);
            seen.push(k.clone());
            fresh
        })
        .map(|(k, d)| format!("{}: {d}", compact(&Value::String(k.clone()))))
        .collect();
    let name: Vec<String> = keys
        .iter()
        .map(|(k, d)| format!("{k}_{d}"))
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut acc, k| {
            if !acc.contains(&k) {
                acc.push(k);
            }
            acc
        });
    format!(
        "{{ \"createIndexes\": {}, \"indexes\": [{{ \"key\": {{ {} }}, \"name\": {} }}] }}",
        compact(&Value::String(collection.to_owned())),
        key.join(", "),
        compact(&Value::String(name.join("_")))
    )
}

fn esr(filter: &[(String, bool)], sort: &[(String, i64)]) -> Vec<(String, i64)> {
    let mut keys: Vec<(String, i64)> = filter
        .iter()
        .filter(|(_, eq)| *eq)
        .map(|(f, _)| (f.clone(), 1))
        .collect();
    keys.extend(sort.iter().cloned());
    keys.extend(
        filter
            .iter()
            .filter(|(_, eq)| !*eq)
            .map(|(f, _)| (f.clone(), 1)),
    );
    keys
}

fn severity(share: Option<f64>, big: bool) -> Severity {
    match share {
        Some(s) if s >= 0.5 => Severity::High,
        Some(s) if s >= 0.1 => Severity::Medium,
        Some(_) => Severity::Low,
        None if big => Severity::High,
        None => Severity::Medium,
    }
}

fn share(plan: &Plan, n: &Node) -> Option<f64> {
    let total = plan.total_ms.filter(|t| *t > 0.0)?;
    Some((n.time_ms? / total).clamp(0.0, 1.0))
}

fn sort_above(plan: &Plan, id: u32) -> Vec<(String, i64)> {
    plan.ancestors(id)
        .find(|a| a.kind == NodeKind::Sort)
        .map(sort_fields)
        .unwrap_or_default()
}

fn full_scan(plan: &Plan, n: &Node, catalog: Option<&Catalog>) -> Option<Finding> {
    let coll = n.relation.clone().unwrap_or_default();
    let examined = n.rows_examined();
    let known = catalog.and_then(|c| c.table(&coll)).and_then(|f| f.rows);
    let size = examined.or(known).unwrap_or(0.0);
    if size < BIG || (plan.under_limit(n.id) && examined.is_some_and(|e| e < BIG)) {
        return None;
    }
    let filter = filter_fields(n);
    let selective = n.rows.is_some_and(|kept| kept <= size * 0.05);
    let level = if selective {
        Severity::High
    } else {
        severity(share(plan, n), true)
    };
    let mut f = Finding::new(Rule::SeqScan, level, Some(n.id));
    f.column = filter.first().map(|(c, _)| c.clone());
    f.rows = Some(size);
    f.kept = n.rows;
    f.share = share(plan, n);
    let keys = esr(&filter, &sort_above(plan, n.id));
    if !keys.is_empty() {
        f.suggestion = Some(create_index(&coll, &keys));
    }
    f.table = Some(coll);
    Some(f)
}

fn loose_fetch(plan: &Plan, n: &Node) -> Option<Finding> {
    let coll = n.relation.clone().unwrap_or_default();
    let (read, back) = (n.rows_examined()?, n.rows?);
    if read < LOOSE_MIN || read < back.max(1.0) * LOOSE {
        return None;
    }
    let filter = filter_fields(n);
    let (column, _) = filter.first().cloned()?;
    let scan = plan.children(n.id).find(|c| c.kind == NodeKind::IndexScan);
    let mut keys: Vec<(String, bool)> = scan
        .and_then(|s| s.details.iter().find(|d| d.key == "keyPattern"))
        .and_then(|d| serde_json::from_str::<Map<String, Value>>(&d.value).ok())
        .map(|m| m.into_iter().map(|(k, _)| (k, true)).collect())
        .unwrap_or_default();
    keys.extend(filter);
    let mut f = Finding::new(
        Rule::DocsExamined,
        severity(share(plan, n), read >= BIG),
        Some(n.id),
    );
    f.column = Some(column);
    f.wrapper = scan.and_then(|s| s.index.clone());
    f.rows = Some(read);
    f.kept = Some(back);
    f.factor = Some(read / back.max(1.0));
    f.share = share(plan, n);
    f.suggestion = Some(create_index(&coll, &esr(&keys, &sort_above(plan, n.id))));
    f.table = Some(coll);
    Some(f)
}

fn memory_sort(plan: &Plan, n: &Node) -> Finding {
    let sorted: f64 = n
        .children
        .iter()
        .filter_map(|c| plan.node(*c))
        .map(|c| c.rows.unwrap_or(0.0))
        .sum();
    let rows = if sorted > 0.0 { Some(sorted) } else { n.rows };
    let scan = plan
        .nodes
        .iter()
        .filter(|d| d.kind.is_scan())
        .find(|d| plan.ancestors(d.id).any(|a| a.id == n.id));
    let coll = scan
        .and_then(|s| s.relation.clone())
        .or_else(|| n.relation.clone())
        .unwrap_or_default();
    let mut filter = scan.map(filter_fields).unwrap_or_default();
    filter.extend(
        scan.and_then(|s| s.parent)
            .and_then(|p| plan.node(p))
            .filter(|p| p.op == "FETCH")
            .map(filter_fields)
            .unwrap_or_default(),
    );
    let mut f = Finding::new(
        Rule::InMemorySort,
        severity(share(plan, n), rows.is_some_and(|r| r >= BIG)),
        Some(n.id),
    );
    f.rows = rows;
    f.kb = n
        .details
        .iter()
        .find(|d| d.key == "totalDataSizeSorted")
        .and_then(|d| d.value.parse::<f64>().ok())
        .map(|b| b / 1024.0);
    f.share = share(plan, n);
    let order = sort_fields(n);
    f.column = order.first().map(|(c, _)| c.clone());
    if !coll.is_empty() && !order.is_empty() && n.op == "SORT" {
        f.suggestion = Some(create_index(&coll, &esr(&filter, &order)));
    }
    f.table = Some(coll).filter(|c| !c.is_empty());
    f
}

pub fn diagnose(plan: &Plan, catalog: Option<&Catalog>) -> Vec<Finding> {
    let mut out: Vec<Finding> = plan
        .nodes
        .iter()
        .filter_map(|n| match n.kind {
            NodeKind::FullScan => full_scan(plan, n, catalog),
            NodeKind::Sort => Some(memory_sort(plan, n)),
            _ if n.op == "FETCH" => loose_fetch(plan, n),
            _ => None,
        })
        .collect();
    out.sort_by_key(|f| f.severity);
    out
}
