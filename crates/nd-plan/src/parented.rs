use std::collections::HashMap;

use crate::model::{Engine, MAX_NODES, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

const ID: usize = 0;
const PARENT: usize = 1;
const OP: usize = 2;
const DETAIL: usize = 3;
const OBJECT: usize = 4;
const SCHEMA: usize = 5;
const ROWS: usize = 6;
const COST: usize = 7;

fn kind(engine: Engine, op: &str) -> NodeKind {
    let o = op.to_ascii_uppercase();
    let has = |w: &str| o.contains(w);
    match engine {
        Engine::Db2 => match o.as_str() {
            "TBSCAN" => NodeKind::FullScan,
            "IXSCAN" | "EISCAN" => NodeKind::IndexScan,
            "FETCH" | "RIDSCN" => NodeKind::BitmapScan,
            "IXAND" => NodeKind::BitmapIndex,
            "HSJOIN" => NodeKind::HashJoin,
            "NLJOIN" => NodeKind::NestedLoop,
            "MSJOIN" | "ZZJOIN" => NodeKind::MergeJoin,
            "SORT" => NodeKind::Sort,
            "GRPBY" => NodeKind::Aggregate,
            "TEMP" => NodeKind::Materialize,
            "UNION" => NodeKind::Append,
            "RETURN" => NodeKind::Result,
            "INSERT" | "UPDATE" | "DELETE" => NodeKind::Modify,
            "TQ" => NodeKind::Gather,
            _ => NodeKind::Other,
        },
        _ if has("HASH JOIN") => NodeKind::HashJoin,
        _ if has("NESTED LOOP") => NodeKind::NestedLoop,
        _ if has("MERGE JOIN") => NodeKind::MergeJoin,
        _ if has("JOIN") => NodeKind::NestedLoop,
        _ if has("INDEX") => NodeKind::IndexScan,
        _ if has("TABLE") || has("SCAN") => NodeKind::FullScan,
        _ if has("AGGREGAT") || has("GROUP") => NodeKind::Aggregate,
        _ if has("ORDER") || has("SORT") => NodeKind::Sort,
        _ if has("LIMIT") => NodeKind::Limit,
        _ if has("UNION") => NodeKind::Append,
        _ if has("INSERT") || has("UPDATE") || has("DELETE") => NodeKind::Modify,
        _ => NodeKind::Other,
    }
}

fn number(s: &str) -> Option<f64> {
    s.trim().parse().ok().filter(|f: &f64| f.is_finite())
}

fn grow(
    i: usize,
    children: &[Vec<usize>],
    built: &mut [Option<Node>],
    seen: &mut [bool],
) -> Option<Tree> {
    if seen[i] {
        return None;
    }
    seen[i] = true;
    let node = built[i].take()?;
    let kids = children[i]
        .iter()
        .filter_map(|c| grow(*c, children, built, seen))
        .collect();
    Some(Tree {
        node,
        children: kids,
    })
}

pub(crate) fn parse(engine: Engine, raw: &str) -> Result<Plan> {
    let mut nodes: Vec<(String, Option<String>, Node)> = Vec::new();
    for line in raw.lines().filter(|l| !l.trim().is_empty()).take(MAX_NODES) {
        let cells: Vec<&str> = line.split('\t').map(str::trim).collect();
        let cell = |i: usize| cells.get(i).copied().filter(|c| !c.is_empty());
        let (Some(id), Some(op)) = (cell(ID), cell(OP)) else {
            continue;
        };
        let mut node = Node::new(op, kind(engine, op));
        let object = cell(OBJECT).map(|o| match cell(SCHEMA) {
            Some(s) if engine == Engine::Hana => format!("{s}.{o}"),
            _ => o.to_owned(),
        });
        if matches!(node.kind, NodeKind::IndexScan | NodeKind::BitmapIndex) && engine == Engine::Db2
        {
            node.index = object;
        } else {
            node.relation = object;
        }
        if let Some(d) = cell(DETAIL) {
            node.detail("details", d);
            if d.to_ascii_uppercase().contains("FILTER") {
                node.filter = Some(d.to_owned());
            }
        }
        node.est_rows = cell(ROWS).and_then(number);
        node.total_cost = cell(COST).and_then(number);
        nodes.push((id.to_owned(), cell(PARENT).map(str::to_owned), node));
    }
    if nodes.is_empty() {
        return Err(PlanError::Shape("no operator rows"));
    }
    let ids: HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| (id.clone(), i))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut roots = Vec::new();
    for (i, (id, parent, _)) in nodes.iter().enumerate() {
        match parent.as_ref().and_then(|p| ids.get(p)) {
            Some(&p) if p != i && parent.as_deref() != Some(id.as_str()) => children[p].push(i),
            _ => roots.push(i),
        }
    }
    let mut built: Vec<Option<Node>> = nodes.into_iter().map(|(_, _, n)| Some(n)).collect();
    let mut seen = vec![false; built.len()];
    let mut trees: Vec<Tree> = roots
        .iter()
        .filter_map(|r| grow(*r, &children, &mut built, &mut seen))
        .collect();
    let root = match trees.len() {
        0 => return Err(PlanError::Shape("the operators form a cycle")),
        1 => trees.remove(0),
        _ => Tree {
            node: Node::new("Query", NodeKind::Result),
            children: trees,
        },
    };
    Ok(Plan::build(engine, false, root, raw))
}
