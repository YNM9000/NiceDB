use crate::model::{Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

fn quoted(s: &str) -> Vec<&str> {
    s.split('"').skip(1).step_by(2).collect()
}

fn node(text: &str) -> Node {
    let names = quoted(text);
    let first = names.first().map(|n| (*n).to_owned());
    let lower = text.to_ascii_lowercase();
    let starts = |p: &str| lower.starts_with(p);
    let kind = if starts("table") || starts("procedure") || starts("local table") {
        if lower.contains("full scan") {
            NodeKind::FullScan
        } else {
            NodeKind::IndexScan
        }
    } else if starts("index") {
        NodeKind::IndexScan
    } else if starts("nested loop join") {
        NodeKind::NestedLoop
    } else if starts("hash join") {
        NodeKind::HashJoin
    } else if starts("merge join") {
        NodeKind::MergeJoin
    } else if starts("aggregate") {
        NodeKind::Aggregate
    } else if starts("unique sort") {
        NodeKind::Unique
    } else if starts("sort") || starts("refetch") {
        NodeKind::Sort
    } else if starts("first n records") || starts("skip n records") {
        NodeKind::Limit
    } else if starts("union") {
        NodeKind::Append
    } else if starts("window") {
        NodeKind::Window
    } else if starts("record buffer") || starts("materialize") {
        NodeKind::Materialize
    } else if starts("sub-query") {
        NodeKind::Subquery
    } else if starts("select expression") || starts("cursor") {
        NodeKind::Result
    } else {
        NodeKind::Other
    };
    let mut n = Node::new(text.to_owned(), kind);
    if starts("index") {
        n.index = first;
    } else if kind.is_scan() {
        n.relation = first;
        n.alias = names.get(1).map(|a| (*a).to_owned());
    }
    n
}

fn attach(stack: &mut Vec<(usize, Tree)>, roots: &mut Vec<Tree>, depth: usize) {
    while let Some((d, _)) = stack.last() {
        if *d < depth {
            break;
        }
        let Some((_, done)) = stack.pop() else { break };
        match stack.last_mut() {
            Some((_, parent)) => parent.children.push(done),
            None => roots.push(done),
        }
    }
}

fn mark_bitmaps(tree: &mut Tree, under_bitmap: bool) {
    let bitmap = tree.node.op.to_ascii_lowercase().starts_with("bitmap");
    if tree.node.kind == NodeKind::IndexScan && tree.node.index.is_some() && under_bitmap {
        tree.node.kind = NodeKind::BitmapIndex;
    }
    if tree.node.relation.is_some()
        && tree
            .children
            .iter()
            .any(|c| c.node.op.to_ascii_lowercase().starts_with("bitmap"))
    {
        tree.node.kind = NodeKind::BitmapScan;
    }
    for c in &mut tree.children {
        mark_bitmaps(c, bitmap || under_bitmap);
    }
}

pub(crate) fn parse(raw: &str) -> Result<Plan> {
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Err(PlanError::Empty);
    }
    if lines.iter().all(|l| l.trim_start().starts_with("PLAN")) {
        let mut root = Tree::leaf(Node::new("Query", NodeKind::Result));
        root.children = lines
            .iter()
            .map(|l| Tree::leaf(Node::new(l.trim(), NodeKind::Other)))
            .collect();
        let root = if root.children.len() == 1 {
            root.children.remove(0)
        } else {
            root
        };
        return Ok(Plan::build(Engine::Firebird, false, root, raw));
    }
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, Tree)> = Vec::new();
    for (count, line) in lines.iter().enumerate() {
        if count >= MAX_NODES {
            break;
        }
        let indent = line.len() - line.trim_start().len();
        let text = line.trim_start().trim_start_matches("->").trim();
        if text.is_empty() {
            continue;
        }
        let depth = (indent / 4).min(MAX_DEPTH as usize);
        attach(&mut stack, &mut roots, depth);
        stack.push((depth, Tree::leaf(node(text))));
    }
    attach(&mut stack, &mut roots, 0);
    let mut root = match roots.len() {
        0 => return Err(PlanError::Shape("no plan lines")),
        1 => roots.remove(0),
        _ => Tree {
            node: Node::new("Query", NodeKind::Result),
            children: roots,
        },
    };
    mark_bitmaps(&mut root, false);
    Ok(Plan::build(Engine::Firebird, false, root, raw))
}
