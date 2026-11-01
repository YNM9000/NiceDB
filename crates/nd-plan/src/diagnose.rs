use std::collections::HashSet;

use nd_sql::Dialect;

use crate::expr::{ColumnUse, Operand, Use, columns};
use crate::model::{Node, NodeKind, SpillKind};
use crate::sqltext::{
    aliases, has_limit, is_query, join_pairs, literal_filters, resolve, selects_star,
};
use crate::{Catalog, Finding, Plan, Rule, Severity, TableFacts};

const BIG: f64 = 10_000.0;
const SELECTIVE: (f64, f64) = (0.05, 0.10);
const HUGE_RESULT: f64 = 100_000.0;
const MANY_LOOPS: f64 = 1_000.0;
const LOOPED_READS: f64 = 5_000_000.0;
const ESTIMATE_OFF: f64 = 10.0;
const WIDE_ROW: f64 = 64.0;
const WIDE_TABLE: usize = 8;

struct Ctx<'a> {
    plan: &'a Plan,
    sql: &'a str,
    catalog: Option<&'a Catalog>,
    dialect: Dialect,
    aliases: Vec<(String, String)>,
}

fn severity(share: Option<f64>, big: bool) -> Severity {
    match share {
        Some(s) if s >= 0.5 => Severity::High,
        Some(s) if s >= 0.1 => Severity::Medium,
        Some(_) => Severity::Low,
        None if big => Severity::High,
        None => Severity::Medium,
    }
}

fn bare(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !matches!(
            name,
            "order"
                | "group"
                | "user"
                | "table"
                | "select"
                | "where"
                | "limit"
                | "desc"
                | "key"
                | "index"
        )
}

impl Ctx<'_> {
    fn ident(&self, name: &str) -> String {
        if bare(name) {
            name.to_owned()
        } else {
            nd_sql::ident(self.dialect, name)
        }
    }

    fn table_of(&self, n: &Node) -> Option<String> {
        n.relation
            .as_deref()
            .map(|r| resolve(&self.aliases, r).to_owned())
    }

    fn facts(&self, table: &str) -> Option<&TableFacts> {
        self.catalog?.table(table)
    }

    fn total(&self) -> Option<f64> {
        self.plan.total_ms.filter(|t| *t > 0.0)
    }

    fn share(&self, ms: Option<f64>) -> Option<f64> {
        Some((ms? / self.total()?).clamp(0.0, 1.0))
    }

    fn mine(&self, n: &Node, u: &ColumnUse) -> bool {
        let Some(q) = u.qualifier.as_deref() else {
            return true;
        };
        let same = |x: Option<&str>| x.is_some_and(|x| x.eq_ignore_ascii_case(q));
        same(n.alias.as_deref())
            || same(n.relation.as_deref())
            || self
                .table_of(n)
                .is_some_and(|t| t.eq_ignore_ascii_case(resolve(&self.aliases, q)))
    }

    fn examined(&self, n: &Node) -> Option<f64> {
        n.rows_examined()
            .or_else(|| n.est_examined.map(|e| e * n.loops.unwrap_or(1.0).max(1.0)))
            .or_else(|| self.table_of(n).and_then(|t| self.facts(&t)?.rows))
    }

    fn subtree(&self, id: u32) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            if let Some(n) = self.plan.node(i) {
                out.push(n);
                stack.extend(n.children.iter().copied());
            }
        }
        out
    }

    fn index_sql(&self, table: &str, key: &str, label: &str) -> String {
        let concurrently = if self.plan.engine == crate::Engine::Postgres {
            "CONCURRENTLY "
        } else {
            ""
        };
        let mut name: String = format!("{table}_{label}_idx")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        while name.contains("__") {
            name = name.replace("__", "_");
        }
        name.truncate(63);
        let qualified = match self.facts(table) {
            Some(f)
                if self.dialect == Dialect::Postgres
                    && !f.namespace.is_empty()
                    && f.namespace != "public" =>
            {
                format!("{}.{}", self.ident(&f.namespace), self.ident(table))
            }
            _ => self.ident(table),
        };
        format!(
            "CREATE INDEX {concurrently}{} ON {qualified} ({key});",
            self.ident(&name)
        )
    }

    fn column_index(&self, table: &str, cols: &[String]) -> String {
        let key = cols
            .iter()
            .map(|c| self.ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        self.index_sql(table, &key, &cols.join("_"))
    }

    fn uses(&self, n: &Node, table: &str) -> Vec<ColumnUse> {
        let mut uses: Vec<ColumnUse> = n
            .filter
            .as_deref()
            .map(columns)
            .unwrap_or_default()
            .into_iter()
            .filter(|u| self.mine(n, u))
            .collect();
        if self.plan.engine == crate::Engine::Sqlite {
            uses.extend(
                literal_filters(self.sql)
                    .into_iter()
                    .filter(|(a, _)| {
                        a.is_empty()
                            || resolve(&self.aliases, a).eq_ignore_ascii_case(table)
                            || n.alias
                                .as_deref()
                                .is_some_and(|x| x.eq_ignore_ascii_case(a))
                    })
                    .map(|(_, column)| ColumnUse {
                        qualifier: None,
                        column,
                        how: Use::Plain {
                            with: Operand::Text,
                        },
                    }),
            );
        }
        uses
    }

    fn wrapping(
        &self,
        table: &str,
        facts: Option<&TableFacts>,
        u: &ColumnUse,
    ) -> Option<(Rule, String, Option<String>)> {
        if !facts.is_some_and(|f| f.leads_index(&u.column)) {
            return None;
        }
        match &u.how {
            Use::Function { name, call } => {
                let call = u
                    .qualifier
                    .as_ref()
                    .map_or_else(|| call.clone(), |q| call.replace(&format!("{q}."), ""));
                let label = format!("{name}_{}", u.column);
                let sql = if self.dialect == Dialect::Mssql {
                    let computed = self.ident(&format!("{}_{name}", u.column));
                    format!(
                        "ALTER TABLE {} ADD {computed} AS {call};\n{}",
                        self.ident(table),
                        self.index_sql(table, &computed, &label)
                    )
                } else {
                    self.index_sql(table, &format!("({call})"), &label)
                };
                Some((Rule::FunctionOnIndexedColumn, name.clone(), Some(sql)))
            }
            Use::Cast { to } => Some((Rule::ImplicitCast, to.clone(), None)),
            Use::Plain {
                with: Operand::Number,
            } if self.dialect == Dialect::Mysql
                && facts.is_some_and(|f| f.is_textual(&u.column)) =>
            {
                Some((Rule::ImplicitCast, "double".to_owned(), None))
            }
            Use::Plain { .. } => None,
        }
    }

    fn scan_rules(&self, n: &Node, out: &mut Vec<Finding>) {
        if n.kind != NodeKind::FullScan {
            return;
        }
        let Some(table) = self.table_of(n) else {
            return;
        };
        let facts = self.facts(&table);
        let indexed = |c: &str| facts.is_some_and(|f| f.leads_index(c));
        let examined = self.examined(n);
        let kept = n.total_rows();
        let share = self.share(n.self_ms.or(n.time_ms));
        let big = examined.unwrap_or(0.0) >= 1_000_000.0;
        let uses = self.uses(n, &table);
        let base = |rule, column: &str| {
            let mut f = Finding::new(rule, severity(share, big), Some(n.id));
            f.table = Some(table.clone());
            f.column = Some(column.to_owned());
            f.rows = examined;
            f.kept = kept;
            f.share = share;
            f
        };
        let wrapped = uses
            .iter()
            .find_map(|u| self.wrapping(&table, facts, u).map(|w| (u, w)));
        if let Some((u, (rule, wrapper, suggestion))) = &wrapped {
            let mut f = base(*rule, &u.column);
            f.wrapper = Some(wrapper.clone());
            f.suggestion.clone_from(suggestion);
            out.push(f);
        }
        let sqlite = self.plan.engine == crate::Engine::Sqlite;
        if wrapped.is_some() || examined.map_or(!sqlite, |e| e < BIG) {
            return;
        }
        let limit = if self.plan.analyzed {
            SELECTIVE.0
        } else {
            SELECTIVE.1
        };
        let selective = match (kept, examined) {
            (Some(k), Some(e)) => k / e.max(1.0) <= limit,
            _ => sqlite,
        };
        if !selective {
            return;
        }
        let mut cols: Vec<String> = Vec::new();
        for u in &uses {
            let constant = matches!(
                u.how,
                Use::Plain {
                    with: Operand::Number | Operand::Text | Operand::Other
                }
            );
            if constant
                && !indexed(&u.column)
                && !cols.iter().any(|c| c.eq_ignore_ascii_case(&u.column))
                && cols.len() < 2
            {
                cols.push(u.column.clone());
            }
        }
        let Some(first) = cols.first() else { return };
        let mut f = base(Rule::SeqScan, first);
        f.suggestion = Some(self.column_index(&table, &cols));
        out.push(f);
    }

    fn join_columns(&self, n: &Node) -> Vec<(String, Option<String>)> {
        let mut conds: Vec<&str> = n.filter.iter().map(String::as_str).collect();
        if let Some(j) = self.plan.ancestors(n.id).find(|a| a.kind.is_join()) {
            conds.extend(j.condition.as_deref());
            conds.extend(j.filter.as_deref());
        }
        let mut out: Vec<(String, Option<String>)> = conds
            .into_iter()
            .flat_map(columns)
            .filter(|u| u.qualifier.is_some() && self.mine(n, u))
            .filter_map(|u| match u.how {
                Use::Plain {
                    with: Operand::Column(Some(q)),
                } => Some((u.column, Some(resolve(&self.aliases, &q).to_owned()))),
                _ => None,
            })
            .collect();
        let table = self.table_of(n).unwrap_or_default();
        for (l, r) in join_pairs(self.sql) {
            for (me, other) in [(&l, &r), (&r, &l)] {
                let mine = resolve(&self.aliases, &me.0).eq_ignore_ascii_case(&table)
                    || n.alias
                        .as_deref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(&me.0));
                if mine {
                    out.push((
                        me.1.clone(),
                        Some(resolve(&self.aliases, &other.0).to_owned()),
                    ));
                }
            }
        }
        out
    }

    fn fk_rule(&self, n: &Node, out: &mut Vec<Finding>) {
        if n.kind != NodeKind::FullScan {
            return;
        }
        let Some(table) = self.table_of(n) else {
            return;
        };
        let Some(facts) = self.facts(&table) else {
            return;
        };
        let examined = self.examined(n);
        if examined.is_some_and(|e| e < BIG) {
            return;
        }
        let join = self.plan.ancestors(n.id).find(|a| a.kind.is_join());
        let narrow = match (join.and_then(Node::total_rows), examined) {
            (Some(o), Some(e)) => o <= 0.2 * e,
            _ => true,
        };
        if !narrow {
            return;
        }
        let Some((column, other)) = self
            .join_columns(n)
            .into_iter()
            .find(|(c, _)| facts.in_foreign_key(c) && !facts.leads_index(c))
        else {
            return;
        };
        let share = self.share(
            n.self_ms
                .or(n.time_ms)
                .map(|s| s + join.and_then(|j| j.self_ms).unwrap_or(0.0)),
        );
        let mut f = Finding::new(
            Rule::FkWithoutIndex,
            severity(share, examined.unwrap_or(0.0) >= 1_000_000.0),
            Some(n.id),
        );
        f.table = Some(table.clone());
        f.suggestion = Some(self.column_index(&table, std::slice::from_ref(&column)));
        f.column = Some(column);
        f.wrapper = other;
        f.rows = examined;
        f.kept = join.and_then(Node::total_rows);
        f.share = share;
        out.push(f);
    }

    fn first_table(&self, id: u32) -> Option<String> {
        self.subtree(id)
            .into_iter()
            .filter(|n| n.kind.is_scan())
            .find_map(|n| self.table_of(n))
    }

    fn estimate_rule(&self, out: &mut Vec<Finding>) {
        if !self.plan.analyzed {
            return;
        }
        let worst = self
            .plan
            .nodes
            .iter()
            .filter(|n| !self.plan.under_limit(n.id) && n.loops.is_some_and(|l| l > 0.0))
            .filter(|n| {
                !matches!(
                    n.kind,
                    NodeKind::Hash
                        | NodeKind::Gather
                        | NodeKind::Limit
                        | NodeKind::Result
                        | NodeKind::Materialize
                )
            })
            .filter_map(|n| {
                let f = n.estimate_factor()?;
                let loops = n.loops.unwrap_or(1.0).max(1.0);
                let size = n.est_rows?.max(n.rows?) * loops;
                (f >= ESTIMATE_OFF && size >= 1_000.0).then_some((n, f))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((n, factor)) = worst else { return };
        let table = self.table_of(n).or_else(|| self.first_table(n.id));
        let mut f = Finding::new(
            Rule::RowEstimate,
            if factor >= 100.0 {
                Severity::Medium
            } else {
                Severity::Low
            },
            Some(n.id),
        );
        let loops = n.loops.unwrap_or(1.0).max(1.0);
        f.rows = n.rows.map(|r| r * loops);
        f.kept = n.est_rows.map(|r| r * loops);
        f.factor = Some(factor);
        f.share = self.share(n.time_ms);
        f.suggestion = table.as_deref().map(|t| match self.dialect {
            Dialect::Mysql => format!("ANALYZE TABLE {};", self.ident(t)),
            Dialect::Mssql => format!("UPDATE STATISTICS {};", self.ident(t)),
            _ => format!("ANALYZE {};", self.ident(t)),
        });
        f.table = table;
        out.push(f);
    }

    fn spill_rule(&self, n: &Node, out: &mut Vec<Finding>) {
        let Some(spill) = &n.spill else { return };
        let share = self.share(n.self_ms);
        let mut f = Finding::new(Rule::Spill, severity(share, false), Some(n.id));
        f.wrapper = Some(
            match spill.kind {
                SpillKind::Sort => "sort",
                SpillKind::Hash => "hash",
                SpillKind::Aggregate => "aggregate",
            }
            .to_owned(),
        );
        f.kb = spill.kb;
        f.share = share;
        f.table = self.first_table(n.id);
        if self.plan.engine == crate::Engine::Postgres {
            let need = spill.kb.map_or(64.0, |kb| kb * 2.0 / 1024.0).max(16.0);
            let mb = 2f64.powf(need.log2().ceil()).min(1024.0);
            f.suggestion = Some(format!("SET work_mem = '{mb:.0}MB';"));
        }
        out.push(f);
    }

    fn loop_rule(&self, out: &mut Vec<Finding>) {
        let mut seen: HashSet<u32> = HashSet::new();
        for n in &self.plan.nodes {
            if seen.contains(&n.id) {
                continue;
            }
            let looped = if self.plan.analyzed {
                let loops = n.loops.unwrap_or(0.0);
                let parent_loops = n
                    .parent
                    .and_then(|p| self.plan.node(p))
                    .and_then(|p| p.loops)
                    .unwrap_or(1.0);
                if loops < 2.0 || loops <= parent_loops {
                    continue;
                }
                let reads: f64 = self
                    .subtree(n.id)
                    .iter()
                    .filter(|x| x.kind.is_scan())
                    .filter_map(|x| x.rows_examined())
                    .sum();
                let share = self.share(n.time_ms).unwrap_or(0.0);
                ((loops >= MANY_LOOPS || reads >= LOOPED_READS) && share >= 0.25)
                    .then_some((loops, reads))
            } else {
                if n.kind != NodeKind::NestedLoop {
                    continue;
                }
                let mut kids = self.plan.children(n.id);
                let (Some(outer), Some(inner)) = (kids.next(), kids.next()) else {
                    continue;
                };
                let outer_rows = outer.total_rows().unwrap_or(0.0);
                let inner_scan = self
                    .subtree(inner.id)
                    .into_iter()
                    .find(|x| x.kind == NodeKind::FullScan);
                let reads = inner_scan.and_then(|s| self.examined(s)).unwrap_or(0.0);
                (outer_rows >= BIG && reads >= BIG).then_some((outer_rows, reads * outer_rows))
            };
            let Some((loops, reads)) = looped else {
                continue;
            };
            let target = if self.plan.analyzed {
                n.id
            } else {
                n.children.get(1).copied().unwrap_or(n.id)
            };
            let nodes = self.subtree(target);
            seen.extend(self.subtree(n.id).iter().map(|x| x.id));
            let mut f = Finding::new(
                Rule::NestedLoop,
                severity(self.share(n.time_ms), true),
                Some(target),
            );
            f.factor = Some(loops);
            f.rows = Some(reads);
            f.share = self.share(self.plan.node(target).and_then(|t| t.time_ms));
            f.table = self.first_table(target);
            f.suggestion = nodes
                .iter()
                .filter(|x| x.kind == NodeKind::FullScan)
                .find_map(|scan| {
                    let table = self.table_of(scan)?;
                    let col = scan
                        .filter
                        .as_deref()
                        .map(columns)
                        .unwrap_or_default()
                        .into_iter()
                        .find(|u| {
                            self.mine(scan, u)
                                && matches!(
                                    u.how,
                                    Use::Plain {
                                        with: Operand::Column(Some(_))
                                    }
                                )
                        })?;
                    let indexed = self
                        .facts(&table)
                        .is_some_and(|t| t.leads_index(&col.column));
                    (!indexed).then(|| self.column_index(&table, &[col.column]))
                });
            out.push(f);
        }
    }

    fn star_rule(&self, out: &mut Vec<Finding>) {
        if !selects_star(self.sql) {
            return;
        }
        for s in self.plan.nodes.iter().filter(|n| n.kind == NodeKind::Sort) {
            let input = self.plan.children(s.id).next();
            let rows = input
                .and_then(Node::total_rows)
                .or_else(|| s.total_rows())
                .unwrap_or(0.0);
            if rows < BIG {
                continue;
            }
            let tables: Vec<(String, usize)> = self
                .subtree(s.id)
                .into_iter()
                .filter(|n| n.kind.is_scan())
                .filter_map(|n| self.table_of(n))
                .map(|t| {
                    let width = self.facts(&t).map_or(0, |f| f.columns.len());
                    (t, width)
                })
                .collect();
            let widest = tables.iter().max_by_key(|(_, w)| *w);
            let wide = s.width.is_some_and(|w| w >= WIDE_ROW)
                || widest.is_some_and(|(_, w)| *w >= WIDE_TABLE);
            if !wide {
                continue;
            }
            let share = self.share(s.self_ms);
            let mut f = Finding::new(Rule::SelectStarSort, severity(share, false), Some(s.id));
            f.table = widest.map(|(t, _)| t.clone());
            f.rows = Some(rows);
            f.factor = s.width;
            f.share = share;
            out.push(f);
            return;
        }
    }

    fn limit_rule(&self, out: &mut Vec<Finding>) {
        if !is_query(self.sql) || has_limit(self.sql) {
            return;
        }
        let Some(rows) = self.plan.root().and_then(Node::total_rows) else {
            return;
        };
        if rows < HUGE_RESULT {
            return;
        }
        let mut f = Finding::new(
            Rule::MissingLimit,
            if rows >= 1_000_000.0 {
                Severity::Medium
            } else {
                Severity::Low
            },
            Some(0),
        );
        f.rows = Some(rows);
        f.table = self
            .plan
            .nodes
            .iter()
            .filter(|n| n.kind.is_scan())
            .max_by(|a, b| {
                a.total_rows()
                    .unwrap_or(0.0)
                    .total_cmp(&b.total_rows().unwrap_or(0.0))
            })
            .and_then(|n| self.table_of(n));
        f.suggestion = Some(format!(
            "{}\nLIMIT 1000",
            self.sql.trim().trim_end_matches(';').trim_end()
        ));
        out.push(f);
    }
}

pub fn diagnose(
    plan: &Plan,
    sql: &str,
    catalog: Option<&Catalog>,
    dialect: Dialect,
) -> Vec<Finding> {
    let ctx = Ctx {
        plan,
        sql,
        catalog,
        dialect,
        aliases: aliases(sql),
    };
    let mut out = Vec::new();
    for n in &plan.nodes {
        ctx.scan_rules(n, &mut out);
        ctx.fk_rule(n, &mut out);
        ctx.spill_rule(n, &mut out);
    }
    ctx.loop_rule(&mut out);
    ctx.estimate_rule(&mut out);
    ctx.star_rule(&mut out);
    ctx.limit_rule(&mut out);
    let mut seen = HashSet::new();
    out.retain(|f| seen.insert((f.rule, f.node, f.column.clone())));
    out.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then(b.share.unwrap_or(0.0).total_cmp(&a.share.unwrap_or(0.0)))
    });
    out
}
