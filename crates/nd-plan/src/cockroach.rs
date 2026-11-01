use crate::model::{Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Spill, SpillKind, Tree};
use crate::{PlanError, Result};

fn number(s: &str) -> Option<f64> {
    let digits: String = s
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, ',' | '.'))
        .filter(|c| *c != ',')
        .collect();
    digits.parse().ok().filter(|f: &f64| f.is_finite())
}

pub(crate) fn duration(s: &str) -> Option<f64> {
    let s = s.trim();
    let mut total = 0.0;
    let mut rest = s;
    let mut any = false;
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let value: f64 = rest[..end].parse().ok()?;
        rest = &rest[end..];
        let unit_end = rest
            .find(|c: char| c.is_ascii_digit() || c.is_whitespace())
            .unwrap_or(rest.len());
        let factor = match &rest[..unit_end] {
            "ns" => 1e-6,
            "µs" | "us" => 1e-3,
            "ms" => 1.0,
            "s" => 1_000.0,
            "m" => 60_000.0,
            "h" => 3_600_000.0,
            _ => return None,
        };
        total += value * factor;
        any = true;
        rest = rest[unit_end..].trim_start();
        if rest.starts_with('(') {
            break;
        }
    }
    any.then_some(total)
}

fn size_kb(s: &str) -> Option<f64> {
    let mut parts = s.split_whitespace();
    let value = number(parts.next()?)?;
    let factor = match parts.next()? {
        "B" => 1.0 / 1024.0,
        "KiB" => 1.0,
        "MiB" => 1024.0,
        "GiB" => 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some(value * factor)
}

fn equality(value: &str) -> String {
    let side = |s: &str| {
        s.trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split(',')
            .map(|c| c.trim().to_owned())
            .collect::<Vec<_>>()
    };
    match value.split_once(" = ") {
        Some((l, r)) => {
            let (l, r) = (side(l), side(r));
            if l.len() == r.len() {
                l.iter()
                    .zip(&r)
                    .map(|(a, b)| format!("{a} = {b}"))
                    .collect::<Vec<_>>()
                    .join(" AND ")
            } else {
                value.to_owned()
            }
        }
        None => value.to_owned(),
    }
}

fn kind(name: &str) -> NodeKind {
    let base = name.split(" (").next().unwrap_or(name);
    match base {
        "scan" | "virtual table" => NodeKind::FullScan,
        "index join" | "vector search" => NodeKind::IndexScan,
        "hash join" => NodeKind::HashJoin,
        "merge join" => NodeKind::MergeJoin,
        "lookup join" | "inverted join" | "cross join" | "apply join" | "zigzag join" => {
            NodeKind::NestedLoop
        }
        "group" => NodeKind::Aggregate,
        "distinct" => NodeKind::Unique,
        "sort" | "top-k" => NodeKind::Sort,
        "limit" => NodeKind::Limit,
        "window" => NodeKind::Window,
        "union all" | "union" | "intersect" | "intersect all" | "except" | "except all" => {
            NodeKind::Append
        }
        "render" | "project" | "values" | "norows" | "emptyrow" => NodeKind::Result,
        "buffer" | "scan buffer" | "with" => NodeKind::Materialize,
        "insert" | "insert fast path" | "update" | "upsert" | "delete" | "delete range" => {
            NodeKind::Modify
        }
        _ => NodeKind::Other,
    }
}

struct Raw {
    column: usize,
    node: Node,
    own_ms: Option<f64>,
}

fn node_line(line: &str) -> Option<(usize, &str)> {
    let at = line.find('•')?;
    let prefix = &line[..at];
    prefix
        .chars()
        .all(|c| matches!(c, ' ' | '│' | '├' | '└' | '─'))
        .then(|| (prefix.chars().count(), line[at + '•'.len_utf8()..].trim()))
}

fn attr_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches([' ', '│', '├', '└', '─']);
    let drawn = line.len() != trimmed.len();
    (drawn || line.starts_with(' ')).then_some(trimmed.trim_end())
}

fn apply(n: &mut Raw, key: &str, value: &str) {
    let node = &mut n.node;
    match key {
        "estimated row count" => node.est_rows = number(value),
        "actual row count" => {
            node.rows = number(value);
            node.loops = Some(1.0);
        }
        "execution time" | "KV time" => {
            if let Some(ms) = duration(value) {
                n.own_ms = Some(n.own_ms.unwrap_or(0.0) + ms);
            }
        }
        "table" => {
            let (table, index) = value.split_once('@').unwrap_or((value, ""));
            node.relation = Some(table.to_owned());
            if !index.is_empty() {
                node.index = Some(index.to_owned());
            }
        }
        "spans" => {
            if node.op.starts_with("scan") {
                node.kind = if value.starts_with("FULL SCAN") {
                    NodeKind::FullScan
                } else {
                    NodeKind::IndexScan
                };
            }
            node.detail(key, value);
        }
        "filter" | "pred" => node.filter = Some(value.to_owned()),
        "equality" => node.condition = Some(equality(value)),
        "lookup condition" | "inverted condition" => {
            node.condition = Some(value.to_owned());
        }
        "estimated max sql temp disk usage" => {
            if let Some(kb) = size_kb(value).filter(|kb| *kb > 0.0) {
                let kind = match node.kind {
                    NodeKind::Sort => SpillKind::Sort,
                    NodeKind::Aggregate | NodeKind::Unique => SpillKind::Aggregate,
                    _ => SpillKind::Hash,
                };
                node.spill = Some(Spill { kind, kb: Some(kb) });
            }
            node.detail(key, value);
        }
        _ => node.detail(key, value),
    }
}

fn fold_filters(mut t: Tree) -> Tree {
    t.children = t.children.into_iter().map(fold_filters).collect();
    let foldable = t.node.op == "filter"
        && t.children.len() == 1
        && t.children[0].node.kind.is_scan()
        && t.children[0].node.filter.is_none();
    if !foldable {
        return t;
    }
    let Some(mut scan) = t.children.pop() else {
        return t;
    };
    let filter = t.node;
    let s = &mut scan.node;
    s.op = format!("{} + filter", s.op);
    s.filter = filter.filter;
    s.est_examined = s.est_rows;
    s.est_rows = filter.est_rows.or(s.est_rows);
    if let (Some(read), Some(kept)) = (s.rows, filter.rows) {
        s.rows_removed = Some((read - kept).max(0.0));
        s.rows = Some(kept);
    }
    s.time_ms = match (s.time_ms, filter.time_ms) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0.0).max(b.unwrap_or(0.0))),
    };
    s.details.extend(filter.details);
    scan
}

fn tree(nodes: Vec<Raw>) -> Option<Tree> {
    let mut stack: Vec<(usize, Tree, Option<f64>)> = Vec::new();
    let mut roots = Vec::new();
    let close = |stack: &mut Vec<(usize, Tree, Option<f64>)>, roots: &mut Vec<Tree>| {
        if let Some((_, mut t, own)) = stack.pop() {
            let below: Option<f64> = t
                .children
                .iter()
                .filter_map(|c| c.node.time_ms)
                .reduce(|a, b| a + b);
            t.node.time_ms = match (own, below) {
                (None, None) => None,
                (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
            };
            if t.node.rows.is_none()
                && t.node.kind == NodeKind::Result
                && let [only] = t.children.as_slice()
            {
                t.node.rows = only.node.rows;
                t.node.loops = only.node.loops;
                t.node.est_rows = t.node.est_rows.or(only.node.est_rows);
            }
            match stack.last_mut() {
                Some(parent) => parent.1.children.push(t),
                None => roots.push(t),
            }
        }
    };
    for raw in nodes {
        while stack.last().is_some_and(|(c, _, _)| *c >= raw.column) {
            close(&mut stack, &mut roots);
        }
        if stack.len() < MAX_DEPTH as usize {
            stack.push((raw.column, Tree::leaf(raw.node), raw.own_ms));
        }
    }
    while !stack.is_empty() {
        close(&mut stack, &mut roots);
    }
    match roots.len() {
        0 => None,
        1 => roots.pop(),
        _ => Some(Tree {
            node: Node::new("plan", NodeKind::Result),
            children: roots,
        }),
    }
}

pub fn parse(raw: &str) -> Result<Plan> {
    let mut nodes: Vec<Raw> = Vec::new();
    let mut header: Vec<(String, String)> = Vec::new();
    let mut advice: Vec<String> = Vec::new();
    let mut in_tree = false;
    let mut after = false;
    for line in raw.lines() {
        if line.trim().is_empty() {
            if in_tree {
                after = true;
            }
            continue;
        }
        if !after && let Some((column, name)) = node_line(line) {
            if nodes.len() >= MAX_NODES {
                break;
            }
            in_tree = true;
            nodes.push(Raw {
                column,
                node: Node::new(name, kind(name)),
                own_ms: None,
            });
            continue;
        }
        if in_tree
            && !after
            && let Some(text) = attr_line(line)
        {
            if text.is_empty() {
                continue;
            }
            if let Some(n) = nodes.last_mut() {
                match text.split_once(": ") {
                    Some((k, v)) => apply(n, k, v.trim()),
                    None if text == "parallel" || text.ends_with(" are key") => {
                        n.node.detail(text, "yes");
                    }
                    None => n.node.detail("note", text),
                }
            }
            continue;
        }
        if in_tree {
            after = true;
            let text = line.trim();
            if let Some((_, sql)) = text
                .split_once("SQL command: ")
                .or_else(|| text.split_once("SQL commands: "))
            {
                advice.push(sql.to_owned());
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(": ") {
            header.push((k.trim().to_owned(), v.trim().to_owned()));
        }
    }
    let analyzed = nodes.iter().any(|n| n.node.rows.is_some());
    let mut root = fold_filters(tree(nodes).ok_or(PlanError::Shape("no plan nodes"))?);
    let mut total_ms = None;
    let mut planning_ms = None;
    for (k, v) in header {
        match k.as_str() {
            "planning time" => planning_ms = duration(&v),
            "execution time" => total_ms = duration(&v),
            _ => root.node.detail(&k, v),
        }
    }
    for sql in advice {
        root.node.detail("index recommendation", sql);
    }
    let mut plan = Plan::build(Engine::Cockroach, analyzed, root, raw);
    plan.planning_ms = planning_ms;
    if total_ms.is_some() {
        plan.total_ms = total_ms;
    }
    Ok(plan)
}
