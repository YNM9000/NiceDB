use crate::model::{Buffers, Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Tree};
use crate::{PlanError, Result};

fn kind(op: &str) -> NodeKind {
    let starts = |p: &str| op.starts_with(p);
    if starts("TABLE ACCESS FULL")
        || starts("TABLE ACCESS STORAGE FULL")
        || starts("MAT_VIEW ACCESS FULL")
    {
        NodeKind::FullScan
    } else if starts("TABLE ACCESS BY")
        || starts("INDEX UNIQUE SCAN")
        || starts("INDEX RANGE SCAN")
        || starts("INDEX SKIP SCAN")
    {
        NodeKind::IndexScan
    } else if starts("INDEX FULL SCAN") || starts("INDEX FAST FULL SCAN") {
        NodeKind::IndexOnlyScan
    } else if starts("BITMAP") {
        NodeKind::BitmapScan
    } else if starts("HASH JOIN") {
        NodeKind::HashJoin
    } else if starts("NESTED LOOPS") {
        NodeKind::NestedLoop
    } else if starts("MERGE JOIN") {
        NodeKind::MergeJoin
    } else if starts("SORT AGGREGATE") || starts("HASH GROUP BY") || starts("SORT GROUP BY") {
        NodeKind::Aggregate
    } else if starts("HASH UNIQUE") || starts("SORT UNIQUE") {
        NodeKind::Unique
    } else if starts("SORT") {
        NodeKind::Sort
    } else if starts("COUNT STOPKEY") {
        NodeKind::Limit
    } else if starts("WINDOW") {
        NodeKind::Window
    } else if starts("UNION") || starts("CONCATENATION") {
        NodeKind::Append
    } else if starts("BUFFER SORT") || starts("TEMP TABLE") {
        NodeKind::Materialize
    } else if starts("PX ") {
        NodeKind::Gather
    } else if starts("VIEW") {
        NodeKind::Subquery
    } else if starts("INSERT")
        || starts("UPDATE")
        || starts("DELETE")
        || starts("MERGE")
        || starts("LOAD ")
    {
        NodeKind::Modify
    } else if op.ends_with("STATEMENT") {
        NodeKind::Result
    } else {
        NodeKind::Other
    }
}

fn scaled(cell: &str) -> Option<f64> {
    let c = cell.trim();
    let (digits, factor) = match c.chars().last()? {
        'K' => (&c[..c.len() - 1], 1e3),
        'M' => (&c[..c.len() - 1], 1e6),
        'G' => (&c[..c.len() - 1], 1e9),
        'T' => (&c[..c.len() - 1], 1e12),
        _ => (c, 1.0),
    };
    digits.trim().parse::<f64>().ok().map(|v| v * factor)
}

fn clock(cell: &str) -> Option<f64> {
    let mut parts = cell.trim().rsplitn(3, ':');
    let seconds: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next().map_or(Ok(0.0), str::parse).ok()?;
    let hours: f64 = parts.next().map_or(Ok(0.0), str::parse).ok()?;
    Some(((hours * 60.0 + minutes) * 60.0 + seconds) * 1000.0)
}

fn cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').collect()
}

fn row(headers: &[String], line: &str) -> Option<(u32, usize, Node)> {
    let c = cells(line);
    let col = |name: &str| {
        headers
            .iter()
            .position(|h| h == name)
            .and_then(|i| c.get(i).copied())
    };
    let id: u32 = col("Id")?
        .trim()
        .trim_start_matches('*')
        .trim()
        .parse()
        .ok()?;
    let raw = col("Operation")?;
    let depth = raw.len() - raw.trim_start().len();
    let op = raw.trim();
    let mut node = Node::new(op.to_owned(), kind(op));
    if let Some(name) = col("Name").map(str::trim).filter(|n| !n.is_empty()) {
        if op.starts_with("INDEX") || op.starts_with("BITMAP INDEX") {
            node.index = Some(name.to_owned());
        } else {
            node.relation = Some(name.to_owned());
        }
    }
    node.est_rows = col("E-Rows").or_else(|| col("Rows")).and_then(scaled);
    node.rows = col("A-Rows").and_then(scaled);
    node.loops = col("Starts").and_then(scaled);
    node.time_ms = col("A-Time").and_then(clock);
    node.total_cost = col("Cost (%CPU)")
        .and_then(|v| v.split('(').next())
        .and_then(scaled);
    if let Some(b) = col("Bytes").and_then(scaled) {
        node.width = node.est_rows.filter(|r| *r > 0.0).map(|r| (b / r).round());
    }
    if let Some(hit) = col("Buffers").and_then(scaled) {
        node.buffers = Some(Buffers {
            hit,
            read: col("Reads").and_then(scaled).unwrap_or(0.0),
            written: col("Writes").and_then(scaled).unwrap_or(0.0),
            temp_read: 0.0,
            temp_written: 0.0,
        });
    }
    node.parallel = op.starts_with("PX ") || col("TQ").is_some_and(|t| !t.trim().is_empty());
    for extra in ["Pstart", "Pstop", "Used-Mem", "Used-Tmp", "E-Time", "Time"] {
        if let Some(v) = col(extra).map(str::trim).filter(|v| !v.is_empty()) {
            node.detail(&extra.to_ascii_lowercase(), v);
        }
    }
    Some((id, depth.saturating_sub(1), node))
}

fn predicates(lines: &[&str]) -> Vec<(u32, bool, String)> {
    let mut out: Vec<(u32, bool, String)> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() || t.starts_with('-') {
            continue;
        }
        let numbered = t
            .split_once(" - ")
            .and_then(|(id, rest)| id.trim().parse::<u32>().ok().map(|id| (id, rest.trim())));
        let (id, text) = match (numbered, out.last()) {
            (Some(found), _) => found,
            (None, Some(last)) if t.starts_with("access(") || t.starts_with("filter(") => {
                (last.0, t)
            }
            (None, Some(_)) => {
                if let Some(last) = out.last_mut() {
                    last.2.push(' ');
                    last.2.push_str(t);
                }
                continue;
            }
            (None, None) => continue,
        };
        out.push((id, text.starts_with("access("), text.to_owned()));
    }
    for p in &mut out {
        p.2 = strip_call(&p.2);
    }
    out
}

fn strip_call(s: &str) -> String {
    let body = s
        .strip_prefix("access(")
        .or_else(|| s.strip_prefix("filter("))
        .unwrap_or(s);
    body.strip_suffix(')').unwrap_or(body).to_owned()
}

pub(crate) fn parse(raw: &str) -> Result<Plan> {
    let lines: Vec<&str> = raw.lines().collect();
    let header = lines
        .iter()
        .position(|l| l.trim_start().starts_with("| Id"))
        .ok_or(PlanError::Shape("no plan table"))?;
    let headers: Vec<String> = cells(lines[header])
        .iter()
        .map(|h| h.trim().to_owned())
        .collect();
    let analyzed = headers.iter().any(|h| h == "A-Rows");
    let mut rows: Vec<(u32, usize, Node)> = Vec::new();
    let mut end = header + 1;
    for (i, line) in lines.iter().enumerate().skip(header + 1) {
        let t = line.trim();
        if t.starts_with('|') {
            if let Some(r) = row(&headers, line) {
                rows.push(r);
            }
            end = i + 1;
        } else if t.starts_with('-') && !rows.is_empty() {
            end = i + 1;
            break;
        }
        if rows.len() >= MAX_NODES {
            break;
        }
    }
    if rows.is_empty() {
        return Err(PlanError::Empty);
    }
    let preds = lines
        .iter()
        .position(|l| l.trim_start().starts_with("Predicate Information"))
        .map(|p| {
            let stop = lines[p + 1..]
                .iter()
                .position(|l| {
                    let t = l.trim_start();
                    !t.is_empty()
                        && !t.starts_with('-')
                        && !t.starts_with(char::is_numeric)
                        && !l.starts_with("  ")
                })
                .map_or(lines.len(), |q| p + 1 + q);
            predicates(&lines[p + 1..stop])
        })
        .unwrap_or_default();
    for (id, access, text) in preds {
        if let Some((_, _, node)) = rows.iter_mut().find(|(i, _, _)| *i == id) {
            let slot = if access {
                &mut node.condition
            } else {
                &mut node.filter
            };
            match slot {
                Some(s) => {
                    s.push_str(" AND ");
                    s.push_str(&text);
                }
                None => *slot = Some(text),
            }
        }
    }
    let notes: Vec<String> = lines[end..]
        .iter()
        .skip_while(|l| !l.trim_start().starts_with("Note"))
        .skip(2)
        .map(|l| l.trim().trim_start_matches("- ").to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    let mut stack: Vec<(usize, Tree)> = Vec::new();
    let mut root: Option<Tree> = None;
    for (_, depth, node) in rows {
        let depth = depth.min(MAX_DEPTH as usize);
        while let Some((d, _)) = stack.last() {
            if *d < depth {
                break;
            }
            let Some((_, done)) = stack.pop() else { break };
            match stack.last_mut() {
                Some((_, parent)) => parent.children.push(done),
                None => root = root.or(Some(done)),
            }
        }
        stack.push((depth, Tree::leaf(node)));
    }
    while let Some((_, done)) = stack.pop() {
        match stack.last_mut() {
            Some((_, parent)) => parent.children.push(done),
            None => root = root.or(Some(done)),
        }
    }
    let mut root = root.ok_or(PlanError::Empty)?;
    for n in notes {
        root.node.detail("note", n);
    }
    Ok(Plan::build(Engine::Oracle, analyzed, root, raw))
}
