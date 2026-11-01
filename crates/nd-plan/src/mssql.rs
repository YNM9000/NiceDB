use std::borrow::Cow;
use std::fmt::Write as _;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::model::{
    Buffers, Engine, MAX_DEPTH, MAX_NODES, Node, NodeKind, Plan, Spill, SpillKind, Tree,
};
use crate::{PlanError, Result};

const MAX_XML_DEPTH: usize = 4_000;
const MAX_ELEMENTS: usize = 500_000;

#[derive(Debug, Default)]
struct El {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<El>,
}

impl El {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    fn num(&self, key: &str) -> Option<f64> {
        self.attr(key)?
            .trim()
            .parse()
            .ok()
            .filter(|f: &f64| f.is_finite())
    }

    fn kid(&self, name: &str) -> Option<&El> {
        self.kids.iter().find(|k| k.name == name)
    }

    fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a El>) {
        for k in &self.kids {
            if k.name == name {
                out.push(k);
            } else if k.name != "RelOp" {
                k.find_all(name, out);
            }
        }
    }

    fn find(&self, name: &str) -> Option<&El> {
        let mut out = Vec::new();
        self.find_all(name, &mut out);
        out.into_iter().next()
    }

    fn deep<'a>(&'a self, name: &str, out: &mut Vec<&'a El>) {
        for k in &self.kids {
            if k.name == name {
                out.push(k);
            }
            k.deep(name, out);
        }
    }
}

fn local(name: &str) -> String {
    name.rsplit_once(':').map_or(name, |(_, l)| l).to_owned()
}

fn element(e: &BytesStart<'_>) -> El {
    El {
        name: local(e.name().as_ref()),
        attrs: e
            .attributes()
            .flatten()
            .map(|a| {
                let value = a
                    .normalized_value(XmlVersion::Implicit1_0)
                    .map_or_else(|_| a.value.clone().into_owned(), Cow::into_owned);
                (local(a.key.as_ref()), value)
            })
            .collect(),
        kids: Vec::new(),
    }
}

fn document(raw: &str) -> Result<El> {
    let mut reader = Reader::from_str(raw);
    let mut stack = vec![El::default()];
    let mut count = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| PlanError::Json(format!("XML: {e}")))?;
        match event {
            Event::Start(e) => {
                count += 1;
                if stack.len() > MAX_XML_DEPTH || count > MAX_ELEMENTS {
                    return Err(PlanError::Shape("the XML plan is too deep or too large"));
                }
                stack.push(element(&e));
            }
            Event::Empty(e) => {
                count += 1;
                if let Some(top) = stack.last_mut() {
                    top.kids.push(element(&e));
                }
            }
            Event::End(_) => {
                if stack.len() > 1
                    && let Some(done) = stack.pop()
                    && let Some(top) = stack.last_mut()
                {
                    top.kids.push(done);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    while stack.len() > 1 {
        if let Some(done) = stack.pop()
            && let Some(top) = stack.last_mut()
        {
            top.kids.push(done);
        }
    }
    stack.pop().ok_or(PlanError::Empty)
}

fn ident(part: &str) -> String {
    let parameter = part.starts_with('@');
    let plain = parameter
        || part
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
            && part.chars().all(|c| c.is_alphanumeric() || c == '_');
    if plain {
        part.to_owned()
    } else {
        format!("\"{}\"", part.replace('"', "\"\""))
    }
}

fn chain(s: &str, mut i: usize) -> (Vec<String>, usize) {
    let b = s.as_bytes();
    let mut parts = Vec::new();
    while b.get(i) == Some(&b'[') {
        let mut j = i + 1;
        let mut part = String::new();
        while j < b.len() {
            if b[j] == b']' {
                if b.get(j + 1) == Some(&b']') {
                    part.push(']');
                    j += 2;
                    continue;
                }
                break;
            }
            let ch = s[j..].chars().next().unwrap_or(' ');
            part.push(ch);
            j += ch.len_utf8();
        }
        parts.push(part);
        i = j + 1;
        if b.get(i) == Some(&b'.') && b.get(i + 1) == Some(&b'[') {
            i += 1;
        } else {
            break;
        }
    }
    (parts, i.min(s.len()))
}

fn short(parts: &[String]) -> String {
    let n = parts.len();
    parts[n.saturating_sub(2)..]
        .iter()
        .map(|p| ident(p))
        .collect::<Vec<_>>()
        .join(".")
}

fn args(s: &str, open: usize) -> Option<(Vec<&str>, usize)> {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut start = open + 1;
    let mut out = Vec::new();
    let mut quoted = false;
    for (j, &c) in b.iter().enumerate().skip(open) {
        if c == b'\'' {
            quoted = !quoted;
        }
        if quoted {
            continue;
        }
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    out.push(s[start..j].trim());
                    return Some((out, j + 1));
                }
            }
            b',' if depth == 1 => {
                out.push(s[start..j].trim());
                start = j + 1;
            }
            _ => {}
        }
    }
    None
}

fn casts(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let upper = rest.to_ascii_uppercase();
        let at = ["CONVERT_IMPLICIT(", "CONVERT("]
            .iter()
            .filter_map(|k| upper.find(k).map(|i| (i, k.len())))
            .filter(|(i, _)| {
                *i == 0
                    || !rest.as_bytes()[i - 1].is_ascii_alphanumeric()
                        && rest.as_bytes()[i - 1] != b'_'
            })
            .min();
        let Some((i, len)) = at else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..i]);
        match args(rest, i + len - 1) {
            Some((a, end)) if a.len() >= 2 => {
                let _ = write!(out, "CAST({} AS {})", casts(a[1]), a[0]);
                rest = &rest[end..];
            }
            _ => {
                out.push_str(&rest[i..i + len]);
                rest = &rest[i + len..];
            }
        }
    }
}

fn without_probes(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("PROBE(") {
        out.push_str(&rest[..i]);
        match args(rest, i + 5) {
            Some((_, end)) => rest = &rest[end..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    let mut s = out.trim().to_owned();
    loop {
        let t = s.trim();
        let t = t
            .strip_prefix("AND ")
            .or_else(|| t.strip_suffix(" AND"))
            .map(str::trim);
        match t {
            Some(t) => s = t.to_owned(),
            None => break,
        }
    }
    s.replace(" AND  AND ", " AND ")
}

pub(crate) fn scalar(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'[' => {
                let (parts, end) = chain(s, i);
                let alias = s[end..]
                    .strip_prefix(" as ")
                    .filter(|r| r.starts_with('['))
                    .map(|_| chain(s, end + 4));
                match alias {
                    Some((a, after)) if !a.is_empty() => {
                        out.push_str(&short(&a));
                        i = after;
                    }
                    _ => {
                        out.push_str(&short(&parts));
                        i = end.max(i + 1);
                    }
                }
            }
            b'\'' => {
                let mut j = i + 1;
                while j < b.len() {
                    if b[j] == b'\'' {
                        if b.get(j + 1) == Some(&b'\'') {
                            j += 2;
                            continue;
                        }
                        break;
                    }
                    j += 1;
                }
                let end = (j + 1).min(b.len());
                out.push_str(&s[i..end]);
                i = end;
            }
            b'N' if b.get(i + 1) == Some(&b'\'')
                && (i == 0 || !b[i - 1].is_ascii_alphanumeric() && b[i - 1] != b'_') =>
            {
                i += 1;
            }
            _ => {
                let ch = s[i..].chars().next().unwrap_or(' ');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    without_probes(&casts(&out))
}

fn column_ref(c: &El) -> String {
    let parts: Vec<String> = match (c.attr("Alias"), c.attr("Table"), c.attr("Column")) {
        (Some(a), _, Some(col)) | (None, Some(a), Some(col)) => {
            vec![unbracket(a), col.to_owned()]
        }
        (_, _, Some(col)) => vec![col.to_owned()],
        _ => Vec::new(),
    };
    parts.iter().map(|p| ident(p)).collect::<Vec<_>>().join(".")
}

fn unbracket(s: &str) -> String {
    s.strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(s)
        .replace("]]", "]")
}

fn columns_of(e: Option<&El>) -> Vec<String> {
    let mut refs = Vec::new();
    if let Some(e) = e {
        e.deep("ColumnReference", &mut refs);
    }
    refs.into_iter().map(column_ref).collect()
}

fn scalar_of(e: Option<&El>) -> Option<String> {
    let op = e?.kid("ScalarOperator")?;
    op.attr("ScalarString")
        .map(scalar)
        .filter(|s| !s.is_empty())
}

fn compare(scan_type: &str) -> &'static str {
    match scan_type {
        "GT" => ">",
        "GE" => ">=",
        "LT" => "<",
        "LE" => "<=",
        "NE" => "<>",
        "IS" => "IS",
        "IS NOT" => "IS NOT",
        _ => "=",
    }
}

fn seek(op: &El) -> Option<String> {
    let mut keys = Vec::new();
    op.find_all("SeekKeys", &mut keys);
    let mut parts = Vec::new();
    for k in keys {
        for range in &k.kids {
            let cols = columns_of(range.kid("RangeColumns"));
            let exprs: Vec<String> = range
                .kid("RangeExpressions")
                .map(|r| {
                    r.kids
                        .iter()
                        .filter_map(|s| s.attr("ScalarString").map(scalar))
                        .collect()
                })
                .unwrap_or_default();
            let sign = compare(range.attr("ScanType").unwrap_or("EQ"));
            for (c, e) in cols.iter().zip(exprs.iter()) {
                parts.push(format!("{c} {sign} {e}"));
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join(" AND "))
}

fn pairs(left: Vec<String>, right: &[String]) -> Option<String> {
    let parts: Vec<String> = left
        .into_iter()
        .zip(right.iter())
        .map(|(l, r)| format!("{l} = {r}"))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" AND "))
}

fn kind(physical: &str, logical: &str) -> NodeKind {
    match physical {
        "Table Scan"
        | "Clustered Index Scan"
        | "Index Scan"
        | "Columnstore Index Scan"
        | "Remote Scan" => NodeKind::FullScan,
        "Index Seek"
        | "Clustered Index Seek"
        | "Key Lookup"
        | "RID Lookup"
        | "Remote Index Seek" => NodeKind::IndexScan,
        "Nested Loops" => NodeKind::NestedLoop,
        "Merge Join" => NodeKind::MergeJoin,
        "Adaptive Join" => NodeKind::HashJoin,
        "Hash Match" => {
            if logical.contains("Join")
                || logical.contains("Semi")
                || logical.contains("Union")
                || logical.contains("Intersect")
                || logical.contains("Except")
            {
                NodeKind::HashJoin
            } else if logical.contains("Distinct") {
                NodeKind::Unique
            } else {
                NodeKind::Aggregate
            }
        }
        "Stream Aggregate" => NodeKind::Aggregate,
        "Window Aggregate" | "Segment" | "Sequence Project" | "Window Spool" => NodeKind::Window,
        "Sort" => {
            if logical.contains("TopN") || logical.contains("Top N") {
                NodeKind::Limit
            } else if logical.contains("Distinct") {
                NodeKind::Unique
            } else {
                NodeKind::Sort
            }
        }
        "Top" => NodeKind::Limit,
        "Parallelism" => NodeKind::Gather,
        "Table Spool" | "Index Spool" | "Row Count Spool" | "Eager Spool" | "Lazy Spool" => {
            NodeKind::Materialize
        }
        "Concatenation" | "Merge Interval" => NodeKind::Append,
        "Constant Scan" | "Compute Scalar" => NodeKind::Result,
        "Table-valued function" | "Remote Query" => NodeKind::Subquery,
        p if p.contains("Insert")
            || p.contains("Update")
            || p.contains("Delete")
            || p.contains("Merge") =>
        {
            NodeKind::Modify
        }
        _ => NodeKind::Other,
    }
}

struct Actual {
    rows: f64,
    read: Option<f64>,
    executions: f64,
    elapsed: Option<f64>,
    cpu: Option<f64>,
    logical: f64,
    physical: f64,
    batch: bool,
    threads: f64,
}

fn actual(relop: &El) -> Option<Actual> {
    let info = relop.kid("RunTimeInformation")?;
    let mut a = Actual {
        rows: 0.0,
        read: None,
        executions: 0.0,
        elapsed: None,
        cpu: None,
        logical: 0.0,
        physical: 0.0,
        batch: false,
        threads: 0.0,
    };
    for t in info
        .kids
        .iter()
        .filter(|k| k.name == "RunTimeCountersPerThread")
    {
        a.rows += t.num("ActualRows").unwrap_or(0.0);
        if let Some(r) = t.num("ActualRowsRead") {
            a.read = Some(a.read.unwrap_or(0.0) + r);
        }
        a.executions += t.num("ActualExecutions").unwrap_or(0.0);
        if let Some(e) = t.num("ActualElapsedms") {
            a.elapsed = Some(a.elapsed.map_or(e, |x: f64| x.max(e)));
        }
        if let Some(c) = t.num("ActualCPUms") {
            a.cpu = Some(a.cpu.unwrap_or(0.0) + c);
        }
        a.logical += t.num("ActualLogicalReads").unwrap_or(0.0);
        a.physical +=
            t.num("ActualPhysicalReads").unwrap_or(0.0) + t.num("ActualReadAheads").unwrap_or(0.0);
        a.batch |= t.attr("ActualExecutionMode") == Some("Batch");
        a.threads += 1.0;
    }
    Some(a)
}

fn spill(relop: &El, kind: NodeKind) -> Option<Spill> {
    let warnings = relop.kid("Warnings")?;
    let details = warnings
        .kids
        .iter()
        .find(|k| k.name.ends_with("SpillDetails"));
    if details.is_none() && warnings.kid("SpillToTempDb").is_none() {
        return None;
    }
    let kb = details
        .and_then(|d| d.num("WritesToTempDb"))
        .map(|pages| pages * 8.0);
    let kind = match (details.map(|d| d.name.as_str()), kind) {
        (Some("SortSpillDetails"), _) | (_, NodeKind::Sort | NodeKind::Limit) => SpillKind::Sort,
        (_, NodeKind::Aggregate | NodeKind::Unique) => SpillKind::Aggregate,
        _ => SpillKind::Hash,
    };
    Some(Spill { kind, kb })
}

fn operator(n: &mut Node, body: &El) {
    n.condition = seek(body)
        .or_else(|| {
            pairs(
                columns_of(body.kid("HashKeysProbe")),
                &columns_of(body.kid("HashKeysBuild")),
            )
        })
        .or_else(|| {
            pairs(
                columns_of(body.kid("InnerSideJoinColumns")),
                &columns_of(body.kid("OuterSideJoinColumns")),
            )
        });
    n.filter = scalar_of(body.kid("Predicate"))
        .or_else(|| scalar_of(body.kid("ProbeResidual")))
        .or_else(|| scalar_of(body.kid("Residual")));
    if let Some(p) =
        scalar_of(body.kid("ProbeResidual")).filter(|_| body.kid("Predicate").is_some())
    {
        n.detail("probe residual", p);
    }
    let outer = columns_of(body.kid("OuterReferences"));
    if !outer.is_empty() {
        n.detail("outer references", outer.join(", "));
    }
    let order = columns_of(body.kid("OrderBy"));
    if !order.is_empty() {
        n.detail("order by", order.join(", "));
    }
    let group = columns_of(body.kid("GroupBy"));
    if !group.is_empty() {
        n.detail("group by", group.join(", "));
    }
    if let Some(top) = scalar_of(body.kid("TopExpression")) {
        n.detail("top", top);
    }
    for key in ["Ordered", "ScanDirection", "Storage", "PartitioningType"] {
        if let Some(v) = body.attr(key) {
            n.detail(key, v);
        }
    }
}

fn measure(n: &mut Node, a: &Actual) {
    let loops = if a.executions > a.threads {
        a.executions
    } else {
        a.executions.min(1.0)
    };
    n.loops = Some(loops);
    n.rows = Some(if loops > 0.0 { a.rows / loops } else { 0.0 });
    if let Some(read) = a.read {
        n.rows_removed = Some(((read - a.rows) / loops.max(1.0)).max(0.0));
    }
    n.time_ms = a.elapsed;
    if let Some(cpu) = a.cpu {
        n.detail("cpu ms", format!("{cpu}"));
    }
    if a.logical > 0.0 || a.physical > 0.0 {
        n.buffers = Some(Buffers {
            hit: (a.logical - a.physical).max(0.0),
            read: a.physical,
            ..Buffers::default()
        });
    }
}

fn relop(e: &El, depth: u32, budget: &mut usize) -> Tree {
    *budget = budget.saturating_sub(1);
    let physical = e.attr("PhysicalOp").unwrap_or("?");
    let logical = e.attr("LogicalOp").unwrap_or_default();
    let kind = kind(physical, logical);
    let mut n = Node::new(physical, kind);
    if !logical.is_empty() && logical != physical {
        n.detail("logical", logical);
    }
    if let Some(obj) = e.find("Object") {
        n.relation = obj.attr("Table").map(unbracket);
        n.alias = obj.attr("Alias").map(unbracket);
        n.index = obj.attr("Index").map(unbracket);
        if let Some(schema) = obj.attr("Schema") {
            n.detail("schema", unbracket(schema));
        }
    }
    let body = e.kids.iter().find(|k| {
        !matches!(
            k.name.as_str(),
            "OutputList"
                | "RunTimeInformation"
                | "Warnings"
                | "MemoryFractions"
                | "RunTimePartitionSummary"
                | "InternalInfo"
        )
    });
    if let Some(body) = body {
        operator(&mut n, body);
    }
    let est_runs =
        1.0 + e.num("EstimateRebinds").unwrap_or(0.0) + e.num("EstimateRewinds").unwrap_or(0.0);
    n.est_rows = e.num("EstimateRows");
    n.est_examined = e.num("EstimatedRowsRead");
    n.total_cost = e.num("EstimatedTotalSubtreeCost");
    n.width = e.num("AvgRowSize");
    n.parallel = matches!(e.attr("Parallel"), Some("1" | "true"));
    if est_runs > 1.0 {
        n.detail("estimated executions", format!("{est_runs}"));
    }
    if let Some(t) = e.num("TableCardinality") {
        n.detail("table rows", format!("{t}"));
    }
    if let Some(m) = e.attr("EstimatedExecutionMode") {
        n.detail("execution mode", m);
    }
    for key in ["EstimateIO", "EstimateCPU"] {
        if let Some(v) = e.attr(key) {
            n.detail(key, v);
        }
    }
    let measured = actual(e);
    if let Some(a) = &measured {
        measure(&mut n, a);
    }
    n.spill = spill(e, kind);
    if let Some(w) = e.kid("Warnings") {
        for k in &w.kids {
            n.detail("warning", k.name.clone());
        }
    }
    let mut kids = Vec::new();
    if depth < MAX_DEPTH {
        for k in &e.kids {
            nested(k, &mut kids);
        }
    }
    let mut children: Vec<Tree> = Vec::new();
    for k in kids {
        if *budget == 0 {
            break;
        }
        children.push(relop(k, depth + 1, budget));
    }
    if let (Some(a), Some(own)) = (&measured, n.time_ms) {
        let times = children.iter().filter_map(|c| c.node.time_ms);
        n.time_ms = Some(if a.batch {
            own + times.sum::<f64>()
        } else {
            times.fold(own, f64::max)
        });
    }
    Tree { node: n, children }
}

fn nested<'a>(e: &'a El, out: &mut Vec<&'a El>) {
    if e.name == "RelOp" {
        out.push(e);
    } else {
        for k in &e.kids {
            nested(k, out);
        }
    }
}

fn fill(t: &mut Tree, params: &[(String, String)]) {
    for text in [&mut t.node.filter, &mut t.node.condition]
        .into_iter()
        .flatten()
    {
        for (name, value) in params {
            *text = text.replace(name.as_str(), value);
        }
    }
    for c in &mut t.children {
        fill(c, params);
    }
}

struct Statement {
    tree: Tree,
    total_ms: Option<f64>,
    planning_ms: Option<f64>,
    analyzed: bool,
}

fn statement(stmt: &El, budget: &mut usize) -> Option<Statement> {
    let qp = stmt.kid("QueryPlan")?;
    let top = qp.kid("RelOp")?;
    let mut tree = relop(top, 1, budget);
    let mut params: Vec<(String, String)> = qp
        .kid("ParameterList")
        .map(|l| {
            l.kids
                .iter()
                .filter_map(|c| {
                    let value = c
                        .attr("ParameterCompiledValue")
                        .or_else(|| c.attr("ParameterRuntimeValue"))?;
                    Some((c.attr("Column")?.to_owned(), scalar(value)))
                })
                .collect()
        })
        .unwrap_or_default();
    params.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    if !params.is_empty() {
        fill(&mut tree, &params);
    }
    let analyzed = tree.node.loops.is_some();
    if let Some(t) = stmt.attr("StatementType") {
        tree.node.detail("statement", t);
    }
    let mut missing = Vec::new();
    qp.deep("MissingIndex", &mut missing);
    for m in missing {
        let table = [m.attr("Schema"), m.attr("Table")]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(".");
        let group = |usage: &str| {
            m.kids
                .iter()
                .filter(|g| g.name == "ColumnGroup" && g.attr("Usage") == Some(usage))
                .flat_map(|g| {
                    g.kids
                        .iter()
                        .filter_map(|c| c.attr("Name").map(str::to_owned))
                })
                .collect::<Vec<_>>()
        };
        let mut key = group("EQUALITY");
        key.extend(group("INEQUALITY"));
        let include = group("INCLUDE");
        let mut sql = format!("CREATE INDEX ON {table} ({})", key.join(", "));
        if !include.is_empty() {
            let _ = write!(sql, " INCLUDE ({})", include.join(", "));
        }
        tree.node.detail("missing index", sql);
    }
    let mut converts = Vec::new();
    qp.deep("PlanAffectingConvert", &mut converts);
    for c in converts {
        if let Some(expr) = c.attr("Expression") {
            tree.node.detail("implicit conversion", scalar(expr));
        }
    }
    if let Some(mem) = qp
        .kid("MemoryGrantInfo")
        .and_then(|g| g.num("GrantedMemory"))
    {
        tree.node.detail("memory grant kb", format!("{mem}"));
    }
    let times = qp.kid("QueryTimeStats");
    Some(Statement {
        tree,
        total_ms: times.and_then(|t| t.num("ElapsedTime")),
        planning_ms: qp.num("CompileTime"),
        analyzed,
    })
}

pub fn parse(raw: &str) -> Result<Plan> {
    let doc = document(raw.trim())?;
    let mut stmts = Vec::new();
    doc.deep("StmtSimple", &mut stmts);
    let mut budget = MAX_NODES;
    let mut found: Vec<Statement> = stmts
        .into_iter()
        .filter_map(|s| statement(s, &mut budget))
        .collect();
    if found.is_empty() {
        return Err(PlanError::Shape("no statement with a query plan"));
    }
    let analyzed = found.iter().any(|s| s.analyzed);
    let (root, total_ms, planning_ms) = if found.len() == 1 {
        let s = found.remove(0);
        (s.tree, s.total_ms, s.planning_ms)
    } else {
        let sum = |f: fn(&Statement) -> Option<f64>| {
            found
                .iter()
                .map(f)
                .try_fold(0.0, |acc, v| v.map(|v| acc + v))
        };
        let (total, planning) = (sum(|s| s.total_ms), sum(|s| s.planning_ms));
        let root = Tree {
            node: Node::new("Batch", NodeKind::Result),
            children: found.into_iter().map(|s| s.tree).collect(),
        };
        (root, total, planning)
    };
    let mut plan = Plan::build(Engine::Mssql, analyzed, root, raw);
    plan.planning_ms = planning_ms;
    if total_ms.is_some() {
        plan.total_ms = total_ms;
    }
    Ok(plan)
}
