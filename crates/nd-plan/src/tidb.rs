use crate::cockroach::duration;
use crate::model::{Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

struct Layout {
    est: usize,
    act: Option<usize>,
    task: usize,
    access: usize,
    exec: Option<usize>,
    info: usize,
    memory: Option<usize>,
}

const BRIEF: Layout = Layout {
    est: 1,
    act: None,
    task: 2,
    access: 3,
    exec: None,
    info: 4,
    memory: None,
};

const ANALYZE: Layout = Layout {
    est: 1,
    act: Some(2),
    task: 3,
    access: 4,
    exec: Some(5),
    info: 6,
    memory: Some(7),
};

fn operator(id: &str) -> &str {
    let id = id.split('(').next().unwrap_or(id);
    match id.rsplit_once('_') {
        Some((op, n)) if n.chars().all(|c| c.is_ascii_digit()) => op,
        _ => id,
    }
}

fn kind(op: &str) -> NodeKind {
    match op {
        "TableFullScan" | "TableRowIDScan" | "MemTableScan" => NodeKind::FullScan,
        "TableRangeScan" | "IndexRangeScan" | "IndexFullScan" | "IndexLookUp" | "IndexMerge"
        | "Point_Get" | "Batch_Point_Get" => NodeKind::IndexScan,
        "IndexReader" => NodeKind::IndexOnlyScan,
        "HashJoin" => NodeKind::HashJoin,
        "MergeJoin" => NodeKind::MergeJoin,
        "IndexJoin" | "IndexHashJoin" | "IndexMergeJoin" | "Apply" => NodeKind::NestedLoop,
        "HashAgg" | "StreamAgg" => NodeKind::Aggregate,
        "Sort" | "TopN" => NodeKind::Sort,
        "Limit" => NodeKind::Limit,
        "Window" => NodeKind::Window,
        "Union" => NodeKind::Append,
        "Projection" | "TableDual" => NodeKind::Result,
        "Insert" | "Update" | "Delete" => NodeKind::Modify,
        "TableReader" | "ExchangeSender" | "ExchangeReceiver" => NodeKind::Gather,
        _ => NodeKind::Other,
    }
}

fn access(node: &mut Node, text: &str) {
    for part in text.split(", ") {
        if let Some(t) = part.strip_prefix("table:") {
            node.relation = Some(t.trim().to_owned());
        } else if let Some(i) = part.strip_prefix("index:") {
            node.index = Some(i.split('(').next().unwrap_or(i).trim().to_owned());
        } else if !part.trim().is_empty() {
            node.detail("access", part.trim());
        }
    }
}

fn split_id(cell: &str) -> (usize, &str) {
    let start = cell
        .char_indices()
        .find(|(_, c)| !matches!(c, ' ' | '│' | '├' | '└' | '─'))
        .map_or(cell.len(), |(i, _)| i);
    (cell[..start].chars().count() / 2, &cell[start..])
}

fn row(cells: &[&str], layout: &Layout) -> Node {
    let get = |i: usize| cells.get(i).copied().unwrap_or("").trim();
    let (_, id) = split_id(cells.first().copied().unwrap_or(""));
    let op = operator(id);
    let mut node = Node::new(id.to_owned(), kind(op));
    node.est_rows = get(layout.est).parse().ok().filter(|f: &f64| f.is_finite());
    if let Some(a) = layout.act {
        node.rows = get(a).parse().ok().filter(|f: &f64| f.is_finite());
    }
    let task = get(layout.task);
    node.parallel = task.starts_with("mpp") || task.starts_with("cop");
    node.detail("task", task);
    access(&mut node, get(layout.access));
    let info = get(layout.info);
    if node.kind.is_join() {
        node.condition = Some(info.to_owned()).filter(|s| !s.is_empty());
    } else if op == "Selection" {
        node.filter = Some(info.to_owned()).filter(|s| !s.is_empty());
    } else {
        node.detail("info", info);
    }
    if let Some(e) = layout.exec {
        let exec = get(e);
        node.time_ms = exec
            .split(", ")
            .find_map(|p| p.strip_prefix("time:"))
            .and_then(duration);
        node.loops = exec
            .split(", ")
            .find_map(|p| p.strip_prefix("loops:"))
            .and_then(|l| l.parse().ok());
        node.detail("execution", exec);
    }
    if let Some(m) = layout.memory {
        let mem = get(m);
        if mem != "N/A" {
            node.detail("memory", mem);
        }
    }
    node
}

pub(crate) fn parse(raw: &str) -> Result<Plan> {
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    let Some(first) = lines.first() else {
        return Err(PlanError::Empty);
    };
    let analyzed = first.split('\t').count() >= 7;
    let layout = if analyzed { &ANALYZE } else { &BRIEF };
    let mut stack: Vec<(usize, Tree)> = Vec::new();
    let mut root: Option<Tree> = None;
    for line in lines.iter().take(MAX_NODES) {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.first().is_some_and(|c| c.trim() == "id") {
            continue;
        }
        let (depth, _) = split_id(cells.first().copied().unwrap_or(""));
        let depth = depth.min(MAX_DEPTH as usize);
        let tree = Tree::leaf(row(&cells, layout));
        while let Some((d, _)) = stack.last() {
            if *d < depth {
                break;
            }
            let (_, done) = stack.pop().ok_or(PlanError::Shape("tree"))?;
            attach(&mut stack, &mut root, done);
        }
        stack.push((depth, tree));
    }
    while let Some((_, done)) = stack.pop() {
        attach(&mut stack, &mut root, done);
    }
    let root = root.ok_or(PlanError::Empty)?;
    Ok(Plan::build(Engine::Tidb, analyzed, root, raw))
}

fn attach(stack: &mut [(usize, Tree)], root: &mut Option<Tree>, done: Tree) {
    match stack.last_mut() {
        Some((_, parent)) => parent.children.push(done),
        None if root.is_none() => *root = Some(done),
        None => {}
    }
}
