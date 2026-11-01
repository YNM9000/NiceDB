use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Postgres,
    Mysql,
    Mariadb,
    Sqlite,
    Mssql,
    Duckdb,
    Cockroach,
    Clickhouse,
    Mongodb,
    Cassandra,
    Elasticsearch,
    Tidb,
    Oracle,
    Firebird,
    Hana,
    Db2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    FullScan,
    IndexScan,
    IndexOnlyScan,
    BitmapScan,
    BitmapIndex,
    HashJoin,
    NestedLoop,
    MergeJoin,
    Hash,
    Aggregate,
    Sort,
    Limit,
    Gather,
    Materialize,
    Subquery,
    Append,
    Window,
    Unique,
    Modify,
    Result,
    Other,
}

impl NodeKind {
    pub fn is_join(self) -> bool {
        matches!(self, Self::HashJoin | Self::NestedLoop | Self::MergeJoin)
    }

    pub fn is_scan(self) -> bool {
        matches!(
            self,
            Self::FullScan
                | Self::IndexScan
                | Self::IndexOnlyScan
                | Self::BitmapScan
                | Self::BitmapIndex
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, specta::Type)]
pub struct Buffers {
    pub hit: f64,
    pub read: f64,
    pub written: f64,
    pub temp_read: f64,
    pub temp_written: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SpillKind {
    Sort,
    Hash,
    Aggregate,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Spill {
    pub kind: SpillKind,
    pub kb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Detail {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Node {
    pub id: u32,
    pub parent: Option<u32>,
    pub depth: u32,
    pub children: Vec<u32>,
    pub op: String,
    pub kind: NodeKind,
    pub relation: Option<String>,
    pub alias: Option<String>,
    pub index: Option<String>,
    pub condition: Option<String>,
    pub filter: Option<String>,
    pub est_rows: Option<f64>,
    pub rows: Option<f64>,
    pub loops: Option<f64>,
    pub rows_removed: Option<f64>,
    pub est_examined: Option<f64>,
    pub startup_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub time_ms: Option<f64>,
    pub self_ms: Option<f64>,
    pub width: Option<f64>,
    pub buffers: Option<Buffers>,
    pub spill: Option<Spill>,
    pub parallel: bool,
    pub details: Vec<Detail>,
}

impl Node {
    pub fn new(op: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id: 0,
            parent: None,
            depth: 0,
            children: Vec::new(),
            op: op.into(),
            kind,
            relation: None,
            alias: None,
            index: None,
            condition: None,
            filter: None,
            est_rows: None,
            rows: None,
            loops: None,
            rows_removed: None,
            est_examined: None,
            startup_cost: None,
            total_cost: None,
            time_ms: None,
            self_ms: None,
            width: None,
            buffers: None,
            spill: None,
            parallel: false,
            details: Vec::new(),
        }
    }

    pub fn detail(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if !value.is_empty() {
            self.details.push(Detail {
                key: key.to_owned(),
                value,
            });
        }
    }

    pub fn total_rows(&self) -> Option<f64> {
        let per_loop = self.rows.or(self.est_rows)?;
        Some(per_loop * self.loops.unwrap_or(1.0).max(1.0))
    }

    pub fn rows_examined(&self) -> Option<f64> {
        let loops = self.loops.unwrap_or(1.0).max(1.0);
        let rows = self.rows?;
        Some((rows + self.rows_removed.unwrap_or(0.0)) * loops)
    }

    pub fn estimate_factor(&self) -> Option<f64> {
        let (est, act) = (self.est_rows?, self.rows?);
        let (lo, hi) = if est < act { (est, act) } else { (act, est) };
        Some(hi.max(1.0) / lo.max(1.0))
    }

    pub fn name(&self) -> Option<&str> {
        self.relation.as_deref().or(self.index.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Plan {
    pub engine: Engine,
    pub analyzed: bool,
    pub nodes: Vec<Node>,
    pub total_ms: Option<f64>,
    pub planning_ms: Option<f64>,
    pub slowest: Option<u32>,
    pub worst_estimate: Option<u32>,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub node: Node,
    pub children: Vec<Tree>,
}

impl Tree {
    pub fn leaf(node: Node) -> Self {
        Self {
            node,
            children: Vec::new(),
        }
    }
}

pub const MAX_DEPTH: u32 = 200;
pub const MAX_NODES: usize = 20_000;

impl Plan {
    pub fn build(engine: Engine, analyzed: bool, root: Tree, raw: &str) -> Self {
        let mut nodes = Vec::new();
        let mut stack = vec![(root, None::<u32>, 0u32)];
        while let Some((tree, parent, depth)) = stack.pop() {
            if nodes.len() >= MAX_NODES {
                break;
            }
            let id = u32::try_from(nodes.len()).unwrap_or(u32::MAX);
            let mut node = tree.node;
            node.id = id;
            node.parent = parent;
            node.depth = depth;
            node.children.clear();
            if let Some(p) = parent.and_then(|p| nodes.get_mut(p as usize)) {
                let p: &mut Node = p;
                p.children.push(id);
            }
            nodes.push(node);
            if depth < MAX_DEPTH {
                for child in tree.children.into_iter().rev() {
                    stack.push((child, Some(id), depth + 1));
                }
            }
        }
        let mut plan = Self {
            engine,
            analyzed,
            nodes,
            total_ms: None,
            planning_ms: None,
            slowest: None,
            worst_estimate: None,
            raw: raw.to_owned(),
        };
        plan.finish();
        plan
    }

    pub fn root(&self) -> Option<&Node> {
        self.nodes.first()
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    pub fn children(&self, id: u32) -> impl Iterator<Item = &Node> {
        self.node(id)
            .into_iter()
            .flat_map(|n| n.children.iter())
            .filter_map(|c| self.node(*c))
    }

    pub fn ancestors(&self, id: u32) -> impl Iterator<Item = &Node> {
        std::iter::successors(self.node(id).and_then(|n| n.parent), |p| {
            self.node(*p).and_then(|n| n.parent)
        })
        .filter_map(|p| self.node(p))
    }

    pub fn under_limit(&self, id: u32) -> bool {
        self.node(id).is_some_and(|n| n.kind == NodeKind::Limit)
            || self
                .ancestors(id)
                .take_while(|a| !matches!(a.kind, NodeKind::Aggregate | NodeKind::Hash))
                .any(|a| a.kind == NodeKind::Limit)
    }

    pub fn finish(&mut self) {
        for i in (0..self.nodes.len()).rev() {
            let Some(own) = self.nodes[i].time_ms else {
                continue;
            };
            let kids: f64 = self.nodes[i]
                .children
                .iter()
                .filter_map(|c| self.nodes.get(*c as usize).and_then(|n| n.time_ms))
                .sum();
            self.nodes[i].self_ms = Some((own - kids).max(0.0));
        }
        if self.total_ms.is_none() {
            self.total_ms = self.root().and_then(|r| r.time_ms);
        }
        self.slowest = self
            .nodes
            .iter()
            .filter_map(|n| n.self_ms.map(|t| (n.id, t)))
            .filter(|(_, t)| t.is_finite())
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
        self.worst_estimate = self
            .nodes
            .iter()
            .filter(|n| !self.under_limit(n.id))
            .filter_map(|n| n.estimate_factor().map(|f| (n.id, f)))
            .filter(|(_, f)| f.is_finite() && *f >= 2.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
    }
}
