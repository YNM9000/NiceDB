use serde_json::{Map, Value};

use crate::model::{Engine, MAX_DEPTH, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

fn json(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|e| PlanError::Json(e.to_string()))
}

fn trace_kind(activity: &str) -> NodeKind {
    let a = activity.to_ascii_lowercase();
    if a.contains("index") {
        NodeKind::IndexScan
    } else if a.contains("range") || a.contains("scanning") || a.contains("scan ") {
        NodeKind::FullScan
    } else if a.contains("single-partition") || a.contains("single partition") {
        NodeKind::IndexScan
    } else if a.contains("merg") {
        NodeKind::Gather
    } else {
        NodeKind::Other
    }
}

pub(crate) fn parse_trace(raw: &str) -> Result<Plan> {
    let v = json(raw)?;
    let events = v
        .get("events")
        .and_then(Value::as_array)
        .ok_or(PlanError::Shape("no events"))?;
    let mut root = Node::new(
        v.get("request")
            .and_then(Value::as_str)
            .unwrap_or("Execute CQL3 query"),
        NodeKind::Result,
    );
    root.time_ms = v
        .get("duration_us")
        .and_then(Value::as_f64)
        .map(|us| us / 1000.0);
    root.detail(
        "coordinator",
        v.get("coordinator")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let mut nodes: Vec<(String, Vec<Tree>, f64)> = Vec::new();
    for e in events {
        let source = e
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned();
        let at = e.get("elapsed_us").and_then(Value::as_f64).unwrap_or(0.0);
        let activity = e.get("activity").and_then(Value::as_str).unwrap_or("?");
        let i = nodes
            .iter()
            .position(|(s, _, _)| *s == source)
            .unwrap_or_else(|| {
                nodes.push((source.clone(), Vec::new(), 0.0));
                nodes.len() - 1
            });
        let (_, events_of_node, previous) = &mut nodes[i];
        let mut n = Node::new(activity, trace_kind(activity));
        n.time_ms = Some(((at - *previous).max(0.0)) / 1000.0);
        n.detail("elapsed_us", format!("{at}"));
        n.detail(
            "thread",
            e.get("thread").and_then(Value::as_str).unwrap_or_default(),
        );
        *previous = at.max(*previous);
        events_of_node.push(Tree::leaf(n));
    }
    let children = nodes
        .into_iter()
        .map(|(source, events, last)| {
            let mut n = Node::new(source.clone(), NodeKind::Gather);
            n.relation = Some(source);
            n.time_ms = Some(last / 1000.0);
            Tree {
                node: n,
                children: events,
            }
        })
        .collect();
    let mut plan = Plan::build(
        Engine::Cassandra,
        true,
        Tree {
            node: root,
            children,
        },
        raw,
    );
    plan.finish();
    Ok(plan)
}

fn nanos(o: &Map<String, Value>) -> Option<f64> {
    o.get("time_in_nanos")
        .and_then(Value::as_f64)
        .map(|n| n / 1_000_000.0)
}

fn query_kind(ty: &str) -> NodeKind {
    match ty {
        "MatchAllDocsQuery" => NodeKind::FullScan,
        t if t.contains("Collector") => NodeKind::Limit,
        t if t.ends_with("Aggregator") || t.contains("Aggregator") => NodeKind::Aggregate,
        t if t.contains("Query") || t.contains("Weight") => NodeKind::IndexScan,
        _ => NodeKind::Other,
    }
}

fn profile_tree(v: &Value, name_key: &str, depth: u32) -> Option<Tree> {
    let o = v.as_object()?;
    let ty = o.get(name_key).and_then(Value::as_str).unwrap_or("?");
    let mut n = Node::new(ty, query_kind(ty));
    n.time_ms = nanos(o);
    if let Some(d) = o.get("description").and_then(Value::as_str) {
        n.condition = Some(d.to_owned());
    }
    if let Some(r) = o.get("reason").and_then(Value::as_str) {
        n.detail("reason", r);
    }
    if let Some(b) = o.get("breakdown").and_then(Value::as_object) {
        for k in [
            "score",
            "build_scorer",
            "next_doc",
            "advance",
            "match",
            "create_weight",
        ] {
            if let Some(x) = b.get(k).and_then(Value::as_f64).filter(|x| *x > 0.0) {
                n.detail(k, format!("{:.3} ms", x / 1_000_000.0));
            }
        }
    }
    let children = if depth < MAX_DEPTH {
        o.get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| profile_tree(c, name_key, depth + 1))
            .collect()
    } else {
        Vec::new()
    };
    Some(Tree { node: n, children })
}

pub(crate) fn parse_profile(raw: &str) -> Result<Plan> {
    let v = json(raw)?;
    let shards = v
        .get("profile")
        .and_then(|p| p.get("shards"))
        .and_then(Value::as_array)
        .ok_or(PlanError::Shape("no profile.shards"))?;
    let mut root = Node::new("search", NodeKind::Result);
    root.time_ms = v.get("took").and_then(Value::as_f64);
    if let Some(h) = v.get("hits").and_then(|h| h.get("total")) {
        root.rows = h
            .get("value")
            .and_then(Value::as_f64)
            .or_else(|| h.as_f64());
    }
    let mut children = Vec::new();
    for s in shards {
        let id = s.get("id").and_then(Value::as_str).unwrap_or("?");
        let mut shard = Node::new(format!("shard {id}"), NodeKind::Gather);
        shard.relation = id.split("][").nth(1).map(str::to_owned);
        let mut parts = Vec::new();
        for search in s
            .get("searches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for q in search
                .get("query")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                parts.extend(profile_tree(q, "type", 1));
            }
            for c in search
                .get("collector")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                parts.extend(profile_tree(c, "name", 1));
            }
        }
        for a in s
            .get("aggregations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(mut t) = profile_tree(a, "type", 1) {
                t.node.kind = NodeKind::Aggregate;
                parts.push(t);
            }
        }
        shard.time_ms = Some(parts.iter().filter_map(|p| p.node.time_ms).sum());
        children.push(Tree {
            node: shard,
            children: parts,
        });
    }
    if root.time_ms.is_none() {
        root.time_ms = Some(
            children
                .iter()
                .filter_map(|c| c.node.time_ms)
                .fold(0.0, f64::max),
        );
    }
    Ok(Plan::build(
        Engine::Elasticsearch,
        true,
        Tree {
            node: root,
            children,
        },
        raw,
    ))
}
