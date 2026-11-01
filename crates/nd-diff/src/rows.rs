use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};

use nd_driver::{Column, Dialect, Row};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RowSide {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RowState {
    Added,
    Removed,
    Changed,
    Same,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ColumnPair {
    pub name: String,
    pub b_name: String,
    pub a_type: Option<String>,
    pub b_type: Option<String>,
    pub type_differs: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct ColumnPlan {
    pub common: Vec<ColumnPair>,
    pub only_a: Vec<Column>,
    pub only_b: Vec<Column>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct CellChange {
    pub column: u32,
    pub was: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RowDiff {
    pub state: RowState,
    pub cells: Row,
    pub changes: Vec<CellChange>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct RowCounts {
    #[specta(type = f64)]
    pub added: u64,
    #[specta(type = f64)]
    pub removed: u64,
    #[specta(type = f64)]
    pub changed: u64,
    #[specta(type = f64)]
    pub same: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    NoKey,
    KeyMissing(String),
    DuplicateKey { side: RowSide, key: Row },
    TooLarge { rows: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Number,
    Bool,
    Time,
    Text,
}

pub(crate) fn kind_of(type_name: Option<&str>) -> Kind {
    let Some(t) = type_name else {
        return Kind::Text;
    };
    let t = t.to_ascii_lowercase();
    let base = t.split('(').next().unwrap_or("").trim();
    let word = base.split_whitespace().next().unwrap_or("");
    match word {
        "int" | "integer" | "smallint" | "bigint" | "tinyint" | "mediumint" | "int2" | "int4"
        | "int8" | "hugeint" | "uhugeint" | "utinyint" | "usmallint" | "uinteger" | "ubigint"
        | "numeric" | "decimal" | "real" | "float" | "float4" | "float8" | "double" | "money"
        | "smallmoney" | "number" | "serial" | "bigserial" | "smallserial" | "oid" => Kind::Number,
        "bool" | "boolean" | "bit" => Kind::Bool,
        "timestamp" | "timestamptz" | "datetime" | "datetime2" | "smalldatetime"
        | "datetimeoffset" | "date" | "time" | "timetz" => Kind::Time,
        _ => Kind::Text,
    }
}

fn merged(a: Kind, b: Kind) -> Kind {
    match (a, b) {
        (x, y) if x == y => x,
        (Kind::Bool, _) | (_, Kind::Bool) => Kind::Bool,
        (Kind::Number, _) | (_, Kind::Number) => Kind::Number,
        (Kind::Time, _) | (_, Kind::Time) => Kind::Time,
        _ => Kind::Text,
    }
}

pub(crate) fn number(s: &str) -> Option<String> {
    let t = s.trim();
    let expanded;
    let t = if t.contains(['e', 'E']) {
        let f: f64 = t.parse().ok()?;
        if !f.is_finite() {
            return None;
        }
        expanded = format!("{f}");
        expanded.as_str()
    } else {
        t
    };
    let (negative, t) = match t.as_bytes().first()? {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let (int, frac) = t.split_once('.').unwrap_or((t, ""));
    if (int.is_empty() && frac.is_empty())
        || !int.bytes().all(|c| c.is_ascii_digit())
        || !frac.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let zero = int == "0" && frac.is_empty();
    let mut out = String::with_capacity(int.len() + frac.len() + 2);
    if negative && !zero {
        out.push('-');
    }
    out.push_str(int);
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    Some(out)
}

pub(crate) fn boolean(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "t" | "true" | "1" | "yes" | "y" | "on" => Some(true),
        "f" | "false" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

fn moment(s: &str) -> Cow<'_, str> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b' ') {
        return Cow::Borrowed(s);
    }
    let mut out = s.to_owned();
    out.replace_range(10..11, " ");
    if out.len() > 19 && out.as_bytes()[19] == b'.' {
        let end = out[20..]
            .find(|c: char| !c.is_ascii_digit())
            .map_or(out.len(), |i| i + 20);
        let digits = out[20..end].trim_end_matches('0').len();
        let cut = if digits == 0 { 19 } else { 20 + digits };
        out.replace_range(cut..end, "");
    }
    Cow::Owned(out)
}

fn normal(kind: Kind, v: &str) -> Cow<'_, str> {
    match kind {
        Kind::Number => number(v).map_or(Cow::Borrowed(v), Cow::Owned),
        Kind::Bool => match boolean(v) {
            Some(true) => Cow::Borrowed("1"),
            Some(false) => Cow::Borrowed("0"),
            None => Cow::Borrowed(v),
        },
        Kind::Time => moment(v),
        Kind::Text => Cow::Borrowed(v),
    }
}

fn equal(kind: Kind, a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b || normal(kind, a) == normal(kind, b),
        _ => false,
    }
}

pub fn plan(a: &[Column], b: &[Column]) -> ColumnPlan {
    let mut taken = vec![false; b.len()];
    let mut common = Vec::new();
    let mut only_a = Vec::new();
    for ca in a {
        let at = b
            .iter()
            .enumerate()
            .position(|(i, cb)| !taken[i] && cb.name == ca.name)
            .or_else(|| {
                b.iter()
                    .enumerate()
                    .position(|(i, cb)| !taken[i] && cb.name.eq_ignore_ascii_case(&ca.name))
            });
        match at {
            Some(i) => {
                taken[i] = true;
                let cb = &b[i];
                common.push(ColumnPair {
                    name: ca.name.clone(),
                    b_name: cb.name.clone(),
                    a_type: ca.type_name.clone(),
                    b_type: cb.type_name.clone(),
                    type_differs: kind_of(ca.type_name.as_deref())
                        != kind_of(cb.type_name.as_deref()),
                });
            }
            None => only_a.push(ca.clone()),
        }
    }
    let only_b = b
        .iter()
        .zip(&taken)
        .filter(|(_, t)| !**t)
        .map(|(c, _)| c.clone())
        .collect();
    ColumnPlan {
        common,
        only_a,
        only_b,
    }
}

struct Pending {
    seq: u64,
    row: Row,
}

fn row_bytes(row: &Row) -> usize {
    64 + row
        .iter()
        .map(|c| 24 + c.as_ref().map_or(0, String::len))
        .sum::<usize>()
}

pub struct Differ {
    plan: ColumnPlan,
    a_index: Vec<usize>,
    b_index: Vec<usize>,
    kinds: Vec<Kind>,
    key: Vec<usize>,
    pending: [HashMap<Box<str>, Pending>; 2],
    matched: [HashSet<u128>; 2],
    hashers: (RandomState, RandomState),
    seq: u64,
    bytes: usize,
    cap: usize,
    keep_same: usize,
    same_cut: bool,
    rows: Vec<RowDiff>,
    counts: RowCounts,
}

impl Differ {
    pub fn new(
        a: &[Column],
        b: &[Column],
        key: &[String],
        cap: usize,
        keep_same: usize,
    ) -> Result<Self, RowError> {
        if key.is_empty() {
            return Err(RowError::NoKey);
        }
        let plan = plan(a, b);
        let index = |cols: &[Column], name: &str| {
            cols.iter()
                .position(|c| c.name == name)
                .expect("a paired column comes from this side")
        };
        let a_index = plan.common.iter().map(|p| index(a, &p.name)).collect();
        let b_index = plan.common.iter().map(|p| index(b, &p.b_name)).collect();
        let kinds = plan
            .common
            .iter()
            .map(|p| merged(kind_of(p.a_type.as_deref()), kind_of(p.b_type.as_deref())))
            .collect();
        let key = key
            .iter()
            .map(|k| {
                plan.common
                    .iter()
                    .position(|p| p.name == *k)
                    .or_else(|| {
                        plan.common
                            .iter()
                            .position(|p| p.name.eq_ignore_ascii_case(k))
                    })
                    .ok_or_else(|| RowError::KeyMissing(k.clone()))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            plan,
            a_index,
            b_index,
            kinds,
            key,
            pending: [HashMap::new(), HashMap::new()],
            matched: [HashSet::new(), HashSet::new()],
            hashers: (RandomState::new(), RandomState::new()),
            seq: 0,
            bytes: 0,
            cap,
            keep_same,
            same_cut: false,
            rows: Vec::new(),
            counts: RowCounts::default(),
        })
    }

    pub fn plan(&self) -> &ColumnPlan {
        &self.plan
    }

    pub fn key(&self) -> &[usize] {
        &self.key
    }

    pub fn rows(&self) -> &[RowDiff] {
        &self.rows
    }

    pub fn counts(&self) -> RowCounts {
        self.counts
    }

    pub fn same_cut(&self) -> bool {
        self.same_cut
    }

    pub fn waiting(&self) -> usize {
        self.pending[0].len() + self.pending[1].len()
    }

    fn encode(&self, row: &Row) -> Box<str> {
        let mut out = String::new();
        for &k in &self.key {
            match row[k].as_deref() {
                None => out.push('\u{1}'),
                Some(v) => {
                    out.push('\u{2}');
                    out.push_str(&normal(self.kinds[k], v));
                }
            }
            out.push('\u{0}');
        }
        out.into_boxed_str()
    }

    fn hash(&self, key: &str) -> u128 {
        let hi = u128::from(self.hashers.0.hash_one(key));
        let lo = u128::from(self.hashers.1.hash_one(key));
        (hi << 64) | lo
    }

    fn key_of(&self, row: &Row) -> Row {
        self.key.iter().map(|&k| row[k].clone()).collect()
    }

    fn keep(&mut self, diff: RowDiff) -> Result<(), RowError> {
        self.bytes += row_bytes(&diff.cells) + diff.changes.len() * 40;
        self.rows.push(diff);
        self.check()
    }

    fn check(&self) -> Result<(), RowError> {
        if self.bytes > self.cap {
            return Err(RowError::TooLarge {
                rows: (self.rows.len() + self.waiting()) as u64,
            });
        }
        Ok(())
    }

    pub fn push(&mut self, side: RowSide, raw: &Row) -> Result<(), RowError> {
        let index = match side {
            RowSide::A => &self.a_index,
            RowSide::B => &self.b_index,
        };
        let row: Row = index
            .iter()
            .map(|&i| raw.get(i).cloned().flatten())
            .collect();
        let key = self.encode(&row);
        let hash = self.hash(&key);
        let (me, other) = match side {
            RowSide::A => (0, 1),
            RowSide::B => (1, 0),
        };
        if self.matched[me].contains(&hash) || self.pending[me].contains_key(&key) {
            return Err(RowError::DuplicateKey {
                side,
                key: self.key_of(&row),
            });
        }
        if let Some(found) = self.pending[other].remove(&key) {
            self.bytes = self.bytes.saturating_sub(row_bytes(&found.row) + key.len());
            self.matched[0].insert(hash);
            self.matched[1].insert(hash);
            self.bytes += 64;
            let (a, b) = match side {
                RowSide::A => (row, found.row),
                RowSide::B => (found.row, row),
            };
            return self.matched_pair(&a, b);
        }
        self.bytes += row_bytes(&row) + key.len();
        self.seq += 1;
        self.pending[me].insert(key, Pending { seq: self.seq, row });
        self.check()
    }

    fn matched_pair(&mut self, a: &Row, b: Row) -> Result<(), RowError> {
        let changes: Vec<CellChange> = a
            .iter()
            .zip(&b)
            .enumerate()
            .filter(|(i, (x, y))| !equal(self.kinds[*i], x.as_deref(), y.as_deref()))
            .map(|(i, (x, _))| CellChange {
                column: u32::try_from(i).unwrap_or(u32::MAX),
                was: x.clone(),
            })
            .collect();
        if changes.is_empty() {
            self.counts.same += 1;
            if self.counts.same <= self.keep_same as u64 {
                return self.keep(RowDiff {
                    state: RowState::Same,
                    cells: b,
                    changes,
                });
            }
            self.same_cut = true;
            return self.check();
        }
        self.counts.changed += 1;
        self.keep(RowDiff {
            state: RowState::Changed,
            cells: b,
            changes,
        })
    }

    pub fn finish(&mut self) {
        for (side, state) in [(0, RowState::Removed), (1, RowState::Added)] {
            let mut left: Vec<Pending> = std::mem::take(&mut self.pending[side])
                .into_values()
                .collect();
            left.sort_unstable_by_key(|p| p.seq);
            for p in left {
                match state {
                    RowState::Removed => self.counts.removed += 1,
                    _ => self.counts.added += 1,
                }
                self.rows.push(RowDiff {
                    state,
                    cells: p.row,
                    changes: Vec::new(),
                });
            }
        }
    }

    pub fn into_parts(self) -> (ColumnPlan, Vec<usize>, Vec<RowDiff>) {
        (self.plan, self.key, self.rows)
    }
}

fn value(dialect: Dialect, kind: Kind, v: Option<&str>) -> String {
    let Some(s) = v else {
        return "NULL".into();
    };
    match kind {
        Kind::Number if number(s).is_some() && !s.trim().contains(['e', 'E']) => {
            s.trim().trim_start_matches('+').to_owned()
        }
        Kind::Bool => match (boolean(s), dialect) {
            (Some(b), Dialect::Postgres | Dialect::Duckdb) => {
                if b { "TRUE" } else { "FALSE" }.into()
            }
            (Some(b), _) => if b { "1" } else { "0" }.into(),
            (None, _) => nd_sql::literal(dialect, Some(s)),
        },
        _ => nd_sql::literal(dialect, Some(s)),
    }
}

const INSERT_ROWS: usize = 100;
const DELETE_KEYS: usize = 500;

pub fn script(
    dialect: Dialect,
    table: &str,
    plan: &ColumnPlan,
    key: &[usize],
    rows: &[RowDiff],
) -> Vec<String> {
    let kinds: Vec<Kind> = plan
        .common
        .iter()
        .map(|p| kind_of(p.a_type.as_deref()))
        .collect();
    let name = |i: usize| nd_sql::ident(dialect, &plan.common[i].name);
    let lit = |i: usize, row: &Row| value(dialect, kinds[i], row[i].as_deref());
    let matches = |row: &Row| {
        key.iter()
            .map(|&k| match row[k] {
                None => format!("{} IS NULL", name(k)),
                Some(_) => format!("{} = {}", name(k), lit(k, row)),
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let of = |state: RowState| rows.iter().filter(move |r| r.state == state);
    let mut out = Vec::new();

    let removed: Vec<&RowDiff> = of(RowState::Removed).collect();
    match key {
        [k] => {
            let (nulls, values): (Vec<&&RowDiff>, Vec<&&RowDiff>) =
                removed.iter().partition(|r| r.cells[*k].is_none());
            if !nulls.is_empty() {
                out.push(format!("DELETE FROM {table} WHERE {} IS NULL", name(*k)));
            }
            for chunk in values.chunks(DELETE_KEYS) {
                let keys = chunk
                    .iter()
                    .map(|r| lit(*k, &r.cells))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(format!(
                    "DELETE FROM {table} WHERE {} IN ({keys})",
                    name(*k)
                ));
            }
        }
        _ => {
            for r in &removed {
                out.push(format!("DELETE FROM {table} WHERE {}", matches(&r.cells)));
            }
        }
    }

    for r in of(RowState::Changed) {
        let set = r
            .changes
            .iter()
            .map(|c| {
                let i = c.column as usize;
                format!("{} = {}", name(i), lit(i, &r.cells))
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push(format!(
            "UPDATE {table} SET {set} WHERE {}",
            matches(&r.cells)
        ));
    }

    let added: Vec<&RowDiff> = of(RowState::Added).collect();
    let columns = (0..plan.common.len())
        .map(name)
        .collect::<Vec<_>>()
        .join(", ");
    for chunk in added.chunks(INSERT_ROWS) {
        let values = chunk
            .iter()
            .map(|r| {
                let cells = (0..plan.common.len())
                    .map(|i| lit(i, &r.cells))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({cells})")
            })
            .collect::<Vec<_>>()
            .join(",\n  ");
        out.push(format!(
            "INSERT INTO {table} ({columns}) VALUES\n  {values}"
        ));
    }
    out
}
