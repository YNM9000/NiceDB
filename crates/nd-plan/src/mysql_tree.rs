use crate::model::{Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

fn field(group: &str, key: &str) -> Option<f64> {
    let at = group.find(&format!("{key}="))? + key.len() + 1;
    let v: String = group[at..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        .collect();
    v.parse().ok().filter(|f: &f64| f.is_finite())
}

fn measures(line: &str) -> (&str, Option<&str>, Option<&str>, bool) {
    let est = line.rfind("(cost=").or_else(|| line.rfind("(rows="));
    let act = line.rfind("(actual time=");
    let never = line.contains("(never executed)");
    let cut = [est, act, line.find("(never executed)")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(line.len());
    let group = |at: Option<usize>| at.map(|a| line[a..].split(')').next().unwrap_or(""));
    (line[..cut].trim_end(), group(est), group(act), never)
}

fn after<'a>(text: &'a str, prefix: &str) -> &'a str {
    text.get(prefix.len()..).unwrap_or("").trim()
}

fn target(rest: &str) -> (String, Option<String>, Option<String>) {
    let (head, cond) = match rest.find(" (").or_else(|| rest.find(" over ")) {
        Some(i) => (
            &rest[..i],
            Some(rest[i..].trim().trim_start_matches("over ").to_owned()),
        ),
        None => (rest, None),
    };
    let mut parts = head.splitn(2, " using ");
    let name = parts.next().unwrap_or("").trim().to_owned();
    let index = parts.next().map(|s| s.trim().to_owned());
    (name, index, cond)
}

pub(crate) fn classify(op: &str) -> Node {
    let lower = op.to_ascii_lowercase();
    let starts = |p: &str| lower.starts_with(p);
    let mut n = Node::new(op, NodeKind::Other);
    if let Some(i) = lower.find(" on ").filter(|_| {
        starts("table scan")
            || starts("index scan")
            || lower.contains("index lookup")
            || lower.contains("index range scan")
            || lower.contains("index skip scan")
            || starts("full-text index search")
            || lower.contains("index point")
            || starts("constant row from")
    }) {
        let (name, index, cond) = target(&op[i + 4..]);
        n.kind = if starts("table scan") || starts("index scan") {
            NodeKind::FullScan
        } else if starts("covering") {
            NodeKind::IndexOnlyScan
        } else {
            NodeKind::IndexScan
        };
        if name.starts_with('<') {
            n.kind = NodeKind::Materialize;
        } else {
            n.relation = Some(name);
        }
        n.index = index;
        n.condition = cond;
        op[..i].clone_into(&mut n.op);
        return n;
    }
    if starts("filter:") {
        n.filter = Some(after(op, "Filter:").to_owned());
        "Filter".clone_into(&mut n.op);
    } else if lower.contains("hash join")
        || lower.contains("hash semijoin")
        || lower.contains("hash antijoin")
    {
        n.kind = NodeKind::HashJoin;
        if let Some(i) = op.find('(') {
            n.condition = Some(op[i..].to_owned());
            op[..i].trim().clone_into(&mut n.op);
        }
    } else if starts("nested loop") {
        n.kind = NodeKind::NestedLoop;
    } else if lower == "hash" {
        n.kind = NodeKind::Hash;
    } else if starts("sort") {
        n.kind = NodeKind::Sort;
        if let Some((name, keys)) = op.split_once(':') {
            n.detail("Sort Key", keys.trim());
            name.trim().clone_into(&mut n.op);
        }
    } else if starts("limit") {
        n.kind = NodeKind::Limit;
        if let Some((name, rows)) = op.split_once(':') {
            n.detail("Limit", rows.trim());
            name.trim().clone_into(&mut n.op);
        }
    } else if lower.contains("aggregate") || starts("group") {
        n.kind = NodeKind::Aggregate;
        if let Some((name, what)) = op.split_once(':') {
            n.detail("Output", what.trim());
            name.trim().clone_into(&mut n.op);
        }
    } else if starts("materialize") || starts("temporary table") {
        n.kind = NodeKind::Materialize;
    } else if starts("select #") || lower.contains("subquery") {
        n.kind = NodeKind::Subquery;
    } else if starts("window") {
        n.kind = NodeKind::Window;
    } else if starts("remove duplicates") || starts("deduplicate") {
        n.kind = NodeKind::Unique;
    } else if starts("union") || starts("append") || starts("intersect") || starts("except") {
        n.kind = NodeKind::Append;
    } else if starts("delete") || starts("update") || starts("insert") {
        n.kind = NodeKind::Modify;
    } else if starts("zero rows") || starts("rows fetched") || starts("constant row") {
        n.kind = NodeKind::Result;
    }
    n
}

fn line_node(text: &str) -> Node {
    let (op, est, act, never) = measures(text);
    let mut n = classify(op);
    if let Some(g) = est {
        n.total_cost = field(g, "cost");
        n.est_rows = field(g, "rows");
    }
    if let Some(g) = act {
        let loops = field(g, "loops").unwrap_or(1.0);
        n.loops = Some(loops);
        n.rows = field(g, "rows");
        let last = g
            .split("..")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|f| f.is_finite());
        n.time_ms = last.map(|t| t * loops);
    } else if never {
        n.loops = Some(0.0);
        n.rows = Some(0.0);
        n.time_ms = Some(0.0);
    }
    n
}

pub(crate) fn fold(mut tree: Tree) -> Tree {
    tree.children = tree.children.into_iter().map(fold).collect();
    let foldable = tree.node.op == "Filter"
        && tree.children.len() == 1
        && tree.children[0].node.kind.is_scan()
        && tree.children[0].node.filter.is_none();
    if !foldable {
        return tree;
    }
    let filter = tree.node;
    let Some(mut scan) = tree.children.pop() else {
        return Tree::leaf(filter);
    };
    let s = &mut scan.node;
    s.est_examined = s.est_rows;
    if let (Some(read), Some(kept)) = (s.rows, filter.rows) {
        s.rows_removed = Some((read - kept).max(0.0));
    }
    s.filter = filter.filter;
    s.est_rows = filter.est_rows.or(s.est_rows);
    s.rows = filter.rows.or(s.rows);
    s.time_ms = filter.time_ms.or(s.time_ms);
    s.loops = filter.loops.or(s.loops);
    scan
}

pub fn parse(raw: &str) -> Result<Plan> {
    let mut stack: Vec<(usize, Tree)> = Vec::new();
    let mut roots: Vec<Tree> = Vec::new();
    let mut count = 0usize;
    let attach = |stack: &mut Vec<(usize, Tree)>, roots: &mut Vec<Tree>, until: usize| {
        while stack.last().is_some_and(|(ind, _)| *ind >= until) {
            let Some((_, done)) = stack.pop() else { break };
            match stack.last_mut() {
                Some((_, parent)) => parent.children.push(done),
                None => roots.push(done),
            }
        }
    };
    for line in raw.lines() {
        let indent = line.len() - line.trim_start().len();
        let body = line.trim();
        let Some(text) = body.strip_prefix("->") else {
            if let Some((_, last)) = stack.last_mut()
                && !body.is_empty()
            {
                last.node.op.push(' ');
                last.node.op.push_str(body);
            }
            continue;
        };
        count += 1;
        if count > MAX_NODES {
            break;
        }
        attach(&mut stack, &mut roots, indent);
        if stack.len() >= MAX_DEPTH as usize {
            continue;
        }
        stack.push((indent, Tree::leaf(line_node(text.trim()))));
    }
    attach(&mut stack, &mut roots, 0);
    let root = match roots.len() {
        0 => return Err(PlanError::Shape("no \"->\" lines")),
        1 => roots.pop().ok_or(PlanError::Empty)?,
        _ => Tree {
            node: Node::new("Query", NodeKind::Result),
            children: roots,
        },
    };
    let root = fold(root);
    let analyzed = raw.contains("(actual time=") || raw.contains("(never executed)");
    Ok(Plan::build(Engine::Mysql, analyzed, root, raw))
}
