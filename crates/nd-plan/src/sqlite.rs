use std::collections::HashMap;

use crate::model::{Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

fn classify(detail: &str) -> Node {
    let d = detail.trim();
    let upper = d.to_ascii_uppercase();
    let mut n = Node::new(d, NodeKind::Other);
    let word = |i: usize| d.split_whitespace().nth(i).map(str::to_owned);
    if upper.starts_with("SCAN ") || upper.starts_with("SEARCH ") {
        let table_at =
            usize::from(upper.starts_with("SCAN TABLE ") || upper.starts_with("SEARCH TABLE ")) + 1;
        n.relation = word(table_at);
        n.op = word(0).unwrap_or_default();
        let using = upper.find(" USING ");
        let automatic = upper.contains("AUTOMATIC");
        let covering = upper.contains("COVERING INDEX");
        n.kind = if upper.starts_with("SCAN") {
            NodeKind::FullScan
        } else if covering {
            NodeKind::IndexOnlyScan
        } else {
            NodeKind::IndexScan
        };
        if let Some(u) = using {
            let rest = &d[u + 7..];
            let (head, cond) = match rest.find(" (") {
                Some(i) => (&rest[..i], Some(rest[i + 1..].trim().to_owned())),
                None => (rest, None),
            };
            n.index = Some(if upper.contains("PRIMARY KEY") {
                "PRIMARY KEY".to_owned()
            } else if automatic {
                "AUTOMATIC".to_owned()
            } else {
                head.split_whitespace().last().unwrap_or(head).to_owned()
            });
            n.condition = cond;
            n.detail("using", head.trim());
        }
        if automatic {
            n.detail("automatic index", "yes");
        }
        return n;
    }
    n.kind = if upper.starts_with("USE TEMP B-TREE FOR GROUP BY") {
        NodeKind::Aggregate
    } else if upper.starts_with("USE TEMP B-TREE FOR DISTINCT") {
        NodeKind::Unique
    } else if upper.starts_with("USE TEMP B-TREE") {
        NodeKind::Sort
    } else if upper.contains("SUBQUERY") || upper.starts_with("CO-ROUTINE") {
        NodeKind::Subquery
    } else if upper.starts_with("MATERIALIZE") {
        NodeKind::Materialize
    } else if upper.starts_with("COMPOUND")
        || upper.starts_with("UNION")
        || upper.starts_with("LEFT-MOST")
    {
        NodeKind::Append
    } else if upper.starts_with("MULTI-INDEX OR") {
        NodeKind::BitmapIndex
    } else {
        NodeKind::Other
    };
    n
}

fn rows(raw: &str) -> Option<Vec<(i64, i64, String)>> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut p = l.splitn(4, '|');
            let id = p.next()?.trim().parse().ok()?;
            let parent = p.next()?.trim().parse().ok()?;
            p.next()?;
            Some((id, parent, p.next()?.to_owned()))
        })
        .collect()
}

fn shell(raw: &str) -> Vec<(i64, i64, String)> {
    let mut out = Vec::new();
    let mut parents: Vec<i64> = vec![0];
    let mut id = 0i64;
    for line in raw.lines() {
        let Some(at) = line.find("--") else { continue };
        let depth = line[..at].chars().count() / 3;
        id += 1;
        parents.truncate(depth + 1);
        let parent = parents.last().copied().unwrap_or(0);
        out.push((id, parent, line[at + 2..].to_owned()));
        parents.push(id);
    }
    out
}

type Children = HashMap<i64, Vec<(i64, String)>>;

fn build(id: i64, detail: String, by_parent: &mut Children, depth: u32) -> Tree {
    let kids = if depth < MAX_DEPTH && id != 0 {
        by_parent.remove(&id).unwrap_or_default()
    } else {
        Vec::new()
    };
    Tree {
        node: classify(&detail),
        children: kids
            .into_iter()
            .map(|(i, d)| build(i, d, by_parent, depth + 1))
            .collect(),
    }
}

pub fn parse(raw: &str) -> Result<Plan> {
    let list = rows(raw)
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| shell(raw));
    if list.is_empty() {
        return Err(PlanError::Shape("no plan rows"));
    }
    let mut by_parent = Children::new();
    for (id, parent, detail) in list.into_iter().take(MAX_NODES) {
        by_parent.entry(parent).or_default().push((id, detail));
    }
    let tops = by_parent.remove(&0).unwrap_or_default();
    let mut roots: Vec<Tree> = tops
        .into_iter()
        .map(|(i, d)| build(i, d, &mut by_parent, 1))
        .collect();
    if roots.is_empty() {
        return Err(PlanError::Shape("no top-level plan row"));
    }
    let root = if roots.len() == 1 {
        roots.pop().ok_or(PlanError::Empty)?
    } else {
        Tree {
            node: Node::new("QUERY PLAN", NodeKind::Result),
            children: roots,
        }
    };
    Ok(Plan::build(Engine::Sqlite, false, root, raw))
}
