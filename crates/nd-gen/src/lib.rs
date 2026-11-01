#![forbid(unsafe_code)]

mod check;
#[rustfmt::skip]
mod data;
#[rustfmt::skip]
mod data_en;
mod guess;
mod it;
mod pattern;
mod person;
mod rng;
mod rule;
mod types;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub use check::{Check, parse_check};
pub use guess::{Facts, guess};
pub use it::{cf_check, codice_fiscale, format_date, partita_iva, piva_check};
pub use pattern::{Expr, ExprError, Template};
pub use rng::Rng;
pub use rule::Rule;
pub use types::{Shape, quoted_list, shape};

use person::Person;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum DataLocale {
    #[default]
    It,
    Uk,
    Us,
}

impl DataLocale {
    pub const ALL: [Self; 3] = [Self::It, Self::Uk, Self::Us];

    fn index(self) -> usize {
        match self {
            Self::It => 0,
            Self::Uk => 1,
            Self::Us => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Dist {
    #[default]
    Uniform,
    Normal,
    Low,
    High,
    Sequential {
        step: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Gen {
    Auto,
    Null,
    Fixed {
        value: String,
    },
    FirstName,
    LastName,
    FullName,
    Company,
    Email,
    Phone,
    Street,
    City,
    Province,
    PostalCode,
    BirthDate,
    CodiceFiscale,
    PartitaIva,
    Integer {
        #[specta(type = f64)]
        min: i64,
        #[specta(type = f64)]
        max: i64,
    },
    Sequence {
        #[specta(type = f64)]
        start: i64,
    },
    Decimal {
        min: f64,
        max: f64,
        scale: u32,
    },
    Boolean,
    Date {
        from: String,
        to: String,
    },
    DateTime {
        from: String,
        to: String,
    },
    Time,
    Lorem {
        min_words: u32,
        max_words: u32,
    },
    Code {
        len: u32,
    },
    Uuid,
    Json,
    Pick {
        values: Vec<String>,
        #[serde(default)]
        weights: Vec<f64>,
    },
    Parent {
        fk: u32,
        column: u32,
    },
    Pattern {
        pattern: String,
    },
    Expr {
        expr: String,
        scale: u32,
    },
}

impl Gen {
    pub fn derived(&self) -> bool {
        matches!(self, Self::Pattern { .. } | Self::Expr { .. })
    }

    fn person(&self) -> bool {
        matches!(
            self,
            Self::FirstName
                | Self::LastName
                | Self::FullName
                | Self::Company
                | Self::Email
                | Self::Phone
                | Self::City
                | Self::Province
                | Self::PostalCode
                | Self::BirthDate
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct ColumnGen {
    pub name: String,
    pub generator: Gen,
    pub null_ratio: f64,
    pub max_len: Option<u32>,
    #[serde(default)]
    pub locale: Option<DataLocale>,
    #[serde(default)]
    pub dist: Dist,
}

impl ColumnGen {
    pub fn new(name: impl Into<String>, generator: Gen) -> Self {
        Self {
            name: name.into(),
            generator,
            null_ratio: 0.0,
            max_len: None,
            locale: None,
            dist: Dist::Uniform,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenError {
    EmptyPool { fk: u32 },
    NoUnique { columns: Vec<String> },
    BadExpr { column: String, message: String },
    Check { expr: String },
}

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPool { fk } => write!(f, "foreign key {fk} has no parent rows"),
            Self::NoUnique { columns } => {
                write!(f, "no new unique value for {}", columns.join(", "))
            }
            Self::BadExpr { column, message } => write!(f, "{column}: {message}"),
            Self::Check { expr } => write!(f, "no generated row satisfies {expr}"),
        }
    }
}

impl std::error::Error for GenError {}

pub(crate) fn plain(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            'à' | 'á' => Some('a'),
            'è' | 'é' => Some('e'),
            'ì' => Some('i'),
            'ò' => Some('o'),
            'ù' => Some('u'),
            c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

fn lorem(r: &mut Rng, words: &[&str], min: u32, max: u32) -> String {
    let n = r.range(i64::from(min.max(1)), i64::from(max.max(min).max(1)));
    let mut list: Vec<&str> = (0..n).map(|_| *r.pick(words)).collect();
    let mut first = list.remove(0).to_owned();
    if let Some(c) = first.get(..1) {
        first = format!("{}{}", c.to_uppercase(), &first[1..]);
    }
    list.insert(0, &first);
    let text = list.join(" ");
    if n > 3 { format!("{text}.") } else { text }
}

fn cut(s: String, max: Option<u32>) -> String {
    let Some(max) = max.map(|m| m as usize) else {
        return s;
    };
    if s.chars().count() <= max {
        return s;
    }
    let mut out: String = s.chars().take(max).collect();
    if let Some(space) = out.rfind(' ').filter(|&p| p * 2 >= max) {
        out.truncate(space);
        while out.ends_with([',', ' ', '.']) {
            out.pop();
        }
    }
    out
}

fn unit(dist: Dist, r: &mut Rng) -> Option<f64> {
    Some(match dist {
        Dist::Uniform | Dist::Sequential { .. } => return None,
        Dist::Normal => loop {
            let (radius, angle) = (r.unit().max(f64::MIN_POSITIVE), r.unit());
            let z = (-2.0 * radius.ln()).sqrt() * (std::f64::consts::TAU * angle).cos();
            let point = 0.5 + z / 6.0;
            if (0.0..1.0).contains(&point) {
                break point;
            }
        },
        Dist::Low => r.unit().powi(3),
        Dist::High => 1.0 - r.unit().powi(3),
    })
}

fn spread(r: &mut Rng, dist: Dist, lo: i64, hi: i64, row: u64) -> i64 {
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    if let Dist::Sequential { step } = dist {
        #[allow(clippy::cast_precision_loss)]
        let span = hi.abs_diff(lo).saturating_add(1) as f64;
        #[allow(clippy::cast_precision_loss)]
        let off = (row as f64 * step).rem_euclid(span);
        #[allow(clippy::cast_possible_truncation)]
        return lo.saturating_add(off.floor() as i64).min(hi);
    }
    match unit(dist, r) {
        None => r.range(lo, hi),
        Some(u) => {
            #[allow(clippy::cast_precision_loss)]
            let span = hi.abs_diff(lo).saturating_add(1) as f64;
            #[allow(clippy::cast_possible_truncation)]
            lo.saturating_add((u * span).floor() as i64).min(hi)
        }
    }
}

fn decimal(rng: &mut Rng, dist: Dist, row: u64, min: f64, max: f64, scale: u32) -> String {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let factor = 10f64.powi(i32::try_from(scale.min(12)).unwrap_or(2));
    #[allow(clippy::cast_possible_truncation)]
    let (low, high) = ((lo * factor).ceil() as i64, (hi * factor).floor() as i64);
    let dist = match dist {
        Dist::Sequential { step } => Dist::Sequential {
            step: step * factor,
        },
        d => d,
    };
    let v = spread(rng, dist, low, high.max(low), row);
    if scale == 0 {
        return v.to_string();
    }
    let s = scale as usize;
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs().to_string();
    let padded = format!("{abs:0>width$}", width = s + 1);
    let (int, frac) = padded.split_at(padded.len() - s);
    format!("{sign}{int}.{frac}")
}

fn date_bounds(from: &str, to: &str) -> (i64, i64) {
    let a = it::parse_date(from).unwrap_or_else(|| it::days_from_civil(2022, 1, 1));
    let b = it::parse_date(to).unwrap_or_else(|| it::days_from_civil(2026, 6, 30));
    (a.min(b), a.max(b))
}

fn clock(r: &mut Rng) -> String {
    let h = r.range(7, 20);
    format!("{h:02}:{:02}:{:02}", r.range(0, 59), r.range(0, 59))
}

fn uuid(r: &mut Rng) -> String {
    let a = r.bits();
    let b = r.bits();
    let b = (b & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000;
    let a = (a & 0xFFFF_FFFF_FFFF_0FFF) | 0x0000_0000_0000_4000;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        a >> 32,
        (a >> 16) & 0xFFFF,
        a & 0xFFFF,
        b >> 48,
        b & 0xFFFF_FFFF_FFFF
    )
}

fn weighted<'a>(r: &mut Rng, values: &'a [String], weights: &[f64]) -> &'a String {
    let w = |i: usize| weights.get(i).copied().unwrap_or(1.0).max(0.0);
    let total: f64 = (0..values.len()).map(w).sum();
    if weights.is_empty() || total <= 0.0 || !total.is_finite() {
        return r.pick(values);
    }
    let mut at = r.unit() * total;
    for (i, v) in values.iter().enumerate() {
        at -= w(i);
        if at < 0.0 {
            return v;
        }
    }
    &values[values.len() - 1]
}

struct Ctx<'a> {
    persons: &'a [Option<Person>; 3],
    row: u64,
    attempt: u32,
    parents: &'a [usize],
    pools: &'a [Vec<Vec<String>>],
}

impl Ctx<'_> {
    fn person(&self, l: DataLocale) -> Option<&Person> {
        self.persons[l.index()].as_ref()
    }
}

fn value(c: &ColumnGen, locale: DataLocale, rng: &mut Rng, cx: &Ctx<'_>) -> Option<String> {
    let who = || cx.person(locale);
    Some(match &c.generator {
        Gen::Auto | Gen::Null | Gen::Pattern { .. } | Gen::Expr { .. } => return None,
        Gen::Fixed { value } => value.clone(),
        Gen::FirstName => who()?.first.to_owned(),
        Gen::LastName => who()?.last.to_owned(),
        Gen::FullName => format!("{} {}", who()?.first, who()?.last),
        Gen::Company => who()?.company(rng),
        Gen::Email => who()?.email(rng, cx.attempt),
        Gen::Phone => who()?.phone(rng),
        Gen::Street => person::street(rng, locale),
        Gen::City => who()?.city().0.to_owned(),
        Gen::Province => who()?.city().1.to_owned(),
        Gen::PostalCode => who()?.postcode(rng),
        Gen::BirthDate => it::format_date(who()?.born),
        Gen::CodiceFiscale => cx.person(DataLocale::It)?.codice_fiscale(),
        Gen::PartitaIva => it::partita_iva(
            u32::try_from(rng.below(10_000_000)).unwrap_or(0),
            u32::try_from(rng.range(1, 100)).unwrap_or(1),
        ),
        Gen::Integer { min, max } => spread(rng, c.dist, *min, *max, cx.row).to_string(),
        Gen::Sequence { start } => start
            .saturating_add(i64::try_from(cx.row).unwrap_or(i64::MAX))
            .to_string(),
        Gen::Decimal { min, max, scale } => decimal(rng, c.dist, cx.row, *min, *max, *scale),
        Gen::Boolean => if rng.chance(0.5) { "true" } else { "false" }.to_owned(),
        Gen::Date { from, to } => {
            let (a, b) = date_bounds(from, to);
            it::format_date(spread(rng, c.dist, a, b, cx.row))
        }
        Gen::DateTime { from, to } => {
            let (a, b) = date_bounds(from, to);
            if let Dist::Sequential { step } = c.dist {
                let hours = spread(
                    rng,
                    Dist::Sequential { step },
                    0,
                    (b - a + 1) * 24 - 1,
                    cx.row,
                );
                let start = 8 + hours;
                return Some(format!(
                    "{} {:02}:00:00",
                    it::format_date(a + start.div_euclid(24)),
                    start.rem_euclid(24)
                ));
            }
            format!(
                "{} {}",
                it::format_date(spread(rng, c.dist, a, b, cx.row)),
                clock(rng)
            )
        }
        Gen::Time => clock(rng),
        Gen::Lorem {
            min_words,
            max_words,
        } => lorem(rng, person::words(locale), *min_words, *max_words),
        Gen::Code { len } => (0..(*len).clamp(1, 64))
            .map(|_| char::from(*rng.pick(data::ALNUM)))
            .collect(),
        Gen::Uuid => uuid(rng),
        Gen::Json => {
            let (note, value) = if locale == DataLocale::It {
                ("nota", "valore")
            } else {
                ("note", "value")
            };
            format!(
                "{{\"{note}\": \"{}\", \"{value}\": {}}}",
                lorem(rng, person::words(locale), 2, 4),
                rng.range(1, 100)
            )
        }
        Gen::Pick { values, .. } if values.is_empty() => return None,
        Gen::Pick { values, weights } => weighted(rng, values, weights).clone(),
        Gen::Parent { fk, column } => {
            let fk = *fk as usize;
            let row = cx.pools.get(fk)?.get(*cx.parents.get(fk)?)?;
            row.get(*column as usize)?.clone()
        }
    })
}

pub fn written(columns: &[ColumnGen]) -> Vec<&ColumnGen> {
    columns
        .iter()
        .filter(|c| c.generator != Gen::Auto)
        .collect()
}

const TRIES: u32 = 200;

type Seen = HashSet<Vec<Option<String>>>;

enum Derived {
    Pattern(Template),
    Expr(Expr, u32),
}

pub struct Generator {
    seed: u64,
    locale: DataLocale,
    year: i32,
    columns: Vec<ColumnGen>,
    derived: Vec<Option<Derived>>,
    fault: Option<GenError>,
    streams: Vec<Rng>,
    picks: Rng,
    unique: Vec<(Vec<usize>, Seen)>,
    pools: Vec<Vec<Vec<String>>>,
    rules: Vec<Rule>,
    row: u64,
}

impl Generator {
    pub fn new(
        seed: u64,
        columns: &[ColumnGen],
        unique: &[Vec<String>],
        pools: Vec<Vec<Vec<String>>>,
    ) -> Self {
        let columns: Vec<ColumnGen> = written(columns).into_iter().cloned().collect();
        let unique = unique
            .iter()
            .filter_map(|set| {
                let at: Option<Vec<usize>> = set
                    .iter()
                    .map(|n| columns.iter().position(|c| &c.name == n))
                    .collect();
                at.filter(|a| !a.is_empty()).map(|a| (a, HashSet::new()))
            })
            .collect();
        let streams = (0..columns.len())
            .map(|i| Rng::derive(seed, i as u64 + 1))
            .collect();
        let names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
        let mut fault = None;
        let derived = columns
            .iter()
            .map(|c| match &c.generator {
                Gen::Pattern { pattern } => {
                    Some(Derived::Pattern(Template::compile(pattern, &names)))
                }
                Gen::Expr { expr, scale } => match Expr::compile(expr, &names) {
                    Ok(e) => Some(Derived::Expr(e, *scale)),
                    Err(ExprError(message)) => {
                        fault.get_or_insert(GenError::BadExpr {
                            column: c.name.clone(),
                            message,
                        });
                        None
                    }
                },
                _ => None,
            })
            .collect();
        Self {
            seed,
            locale: DataLocale::It,
            year: 2026,
            columns,
            derived,
            fault,
            streams,
            picks: Rng::derive(seed, 1 << 32),
            unique,
            pools,
            rules: Vec::new(),
            row: 0,
        }
    }

    #[must_use]
    pub fn rules(mut self, rules: Vec<Rule>) -> Self {
        self.rules = rules;
        self
    }

    pub fn taken(&mut self, set: &[String], values: impl IntoIterator<Item = Vec<Option<String>>>) {
        let at: Option<Vec<usize>> = set
            .iter()
            .map(|n| self.columns.iter().position(|c| &c.name == n))
            .collect();
        let Some(at) = at else { return };
        if let Some((_, seen)) = self.unique.iter_mut().find(|(a, _)| *a == at) {
            seen.extend(
                values
                    .into_iter()
                    .filter(|v| v.len() == at.len() && v.iter().all(Option::is_some)),
            );
        }
    }

    #[must_use]
    pub fn locale(mut self, locale: DataLocale) -> Self {
        self.locale = locale;
        self
    }

    #[must_use]
    pub fn year(mut self, year: i32) -> Self {
        self.year = year;
        self
    }

    pub fn columns(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }

    fn parents_used(&self) -> Vec<u32> {
        let mut fks: Vec<u32> = self
            .columns
            .iter()
            .filter_map(|c| match c.generator {
                Gen::Parent { fk, .. } if c.null_ratio < 1.0 => Some(fk),
                _ => None,
            })
            .collect();
        fks.sort_unstable();
        fks.dedup();
        fks
    }

    fn locale_of(&self, c: &ColumnGen) -> DataLocale {
        if matches!(c.generator, Gen::CodiceFiscale) {
            return DataLocale::It;
        }
        c.locale.unwrap_or(self.locale)
    }

    fn persons(&self, attempt: u32) -> [Option<Person>; 3] {
        let mut out = [None, None, None];
        let mut it = Rng::derive(self.seed, (2 << 32) + self.row * 1024 + u64::from(attempt));
        out[0] = Some(Person::new(&mut it, DataLocale::It));
        for l in [DataLocale::Uk, DataLocale::Us] {
            let used = self
                .columns
                .iter()
                .any(|c| c.generator.person() && self.locale_of(c) == l);
            if used {
                let part = ((3 + l.index() as u64) << 40) + self.row * 1024 + u64::from(attempt);
                out[l.index()] = Some(Person::new(&mut Rng::derive(self.seed, part), l));
            }
        }
        out
    }

    pub fn next_row(&mut self) -> Result<Vec<Option<String>>, GenError> {
        if let Some(f) = &self.fault {
            return Err(f.clone());
        }
        for fk in self.parents_used() {
            if self.pools.get(fk as usize).is_none_or(Vec::is_empty) {
                return Err(GenError::EmptyPool { fk });
            }
        }
        let mut last = None;
        let mut broken = None;
        for attempt in 0..TRIES {
            let persons = self.persons(attempt);
            let parents: Vec<usize> = self
                .pools
                .iter()
                .map(|p| self.picks.index(p.len()))
                .collect();
            let cx = Ctx {
                persons: &persons,
                row: self.row,
                attempt,
                parents: &parents,
                pools: &self.pools,
            };
            let locales: Vec<DataLocale> = self.columns.iter().map(|c| self.locale_of(c)).collect();
            let mut row: Vec<Option<String>> = self
                .columns
                .iter()
                .zip(self.streams.iter_mut())
                .zip(&locales)
                .map(|((c, r), l)| {
                    if c.generator.derived() || r.chance(c.null_ratio) {
                        return None;
                    }
                    value(c, *l, r, &cx).map(|v| cut(v, c.max_len))
                })
                .collect();
            for i in 0..self.columns.len() {
                let Some(d) = &self.derived[i] else { continue };
                let c = &self.columns[i];
                let r = &mut self.streams[i];
                if r.chance(c.null_ratio) {
                    continue;
                }
                let v = match d {
                    Derived::Pattern(t) => Some(t.render(r, self.row, self.year, &row)),
                    Derived::Expr(e, scale) => e.render(&row, *scale),
                };
                row[i] = v.map(|v| cut(v, c.max_len));
            }
            if let Some(k) = self.rules.iter().position(|r| !r.holds(&row)) {
                broken = Some(k);
                continue;
            }
            let clash = self.unique.iter().position(|(at, seen)| {
                let key: Vec<Option<String>> = at.iter().map(|&i| row[i].clone()).collect();
                key.iter().all(Option::is_some) && seen.contains(&key)
            });
            if clash.is_some() {
                last = clash;
                continue;
            }
            for (at, seen) in &mut self.unique {
                let key: Vec<Option<String>> = at.iter().map(|&i| row[i].clone()).collect();
                if key.iter().all(Option::is_some) {
                    seen.insert(key);
                }
            }
            self.row += 1;
            return Ok(row);
        }
        if let (None, Some(k)) = (last, broken) {
            return Err(GenError::Check {
                expr: self.rules[k].text.clone(),
            });
        }
        let columns = last
            .and_then(|i| self.unique.get(i))
            .map(|(at, _)| at.iter().map(|&i| self.columns[i].name.clone()).collect())
            .unwrap_or_default();
        Err(GenError::NoUnique { columns })
    }
}
