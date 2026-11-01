use serde::Serialize;

use crate::{DdlKind, ObjectRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DepDirection {
    Uses,
    UsedBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DepKind {
    Table,
    View,
    Routine,
    Trigger,
    Sequence,
}

impl DepKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "table" => Some(Self::Table),
            "view" => Some(Self::View),
            "routine" => Some(Self::Routine),
            "trigger" => Some(Self::Trigger),
            "sequence" => Some(Self::Sequence),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DepVia {
    Definition,
    ForeignKey,
    Trigger,
    Owned,
}

impl DepVia {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "definition" => Some(Self::Definition),
            "foreign_key" => Some(Self::ForeignKey),
            "trigger" => Some(Self::Trigger),
            "owned" => Some(Self::Owned),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
pub struct Dependency {
    pub direction: DepDirection,
    pub via: DepVia,
    pub kind: DepKind,
    pub namespace: String,
    pub name: String,
    pub detail: Option<String>,
}

impl Dependency {
    pub fn from_row(row: &[Option<String>]) -> Option<Self> {
        let text = |i: usize| row.get(i).cloned().flatten();
        Some(Self {
            direction: match text(0)?.as_str() {
                "uses" => DepDirection::Uses,
                "used_by" => DepDirection::UsedBy,
                _ => return None,
            },
            via: DepVia::parse(&text(1)?)?,
            kind: DepKind::parse(&text(2)?)?,
            namespace: text(3).unwrap_or_default(),
            name: text(4)?,
            detail: text(5),
        })
    }
}

pub fn tidy(mut deps: Vec<Dependency>) -> Vec<Dependency> {
    deps.sort();
    deps.dedup();
    deps
}

pub fn mentions(sql: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let hay = sql.to_lowercase();
    let needle = name.to_lowercase();
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$');
    hay.match_indices(&needle).any(|(at, _)| {
        let before = hay[..at].chars().next_back();
        let after = hay[at + needle.len()..].chars().next();
        !word(before) && !word(after)
    })
}

pub fn by_text(object: &ObjectRef, rows: &[Vec<Option<String>>]) -> Vec<Dependency> {
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let is_it = |ns: &str, name: &str| same(ns, &object.namespace) && same(name, &object.name);
    let cell = |r: &Vec<Option<String>>, i: usize| {
        r.get(i)
            .and_then(Option::as_deref)
            .unwrap_or_default()
            .to_owned()
    };
    let dep = |direction, via, kind, namespace: String, name: String, detail| Dependency {
        direction,
        via,
        kind,
        namespace,
        name,
        detail,
    };
    let mut out = Vec::new();
    let mut own = None;
    for r in rows {
        let role = cell(r, 0);
        if role == "fk" {
            let detail = r.get(5).cloned().flatten();
            if is_it(&cell(r, 1), &cell(r, 2)) {
                out.push(dep(
                    DepDirection::Uses,
                    DepVia::ForeignKey,
                    DepKind::Table,
                    cell(r, 3),
                    cell(r, 4),
                    detail.clone(),
                ));
            }
            if is_it(&cell(r, 3), &cell(r, 4)) {
                out.push(dep(
                    DepDirection::UsedBy,
                    DepVia::ForeignKey,
                    DepKind::Table,
                    cell(r, 1),
                    cell(r, 2),
                    detail,
                ));
            }
            continue;
        }
        let Some(kind) = DepKind::parse(&cell(r, 1)) else {
            continue;
        };
        let (ns, name, sql) = (cell(r, 2), cell(r, 3), cell(r, 5));
        if is_it(&ns, &name) {
            own = Some(sql);
        } else if kind == DepKind::Trigger && same(&cell(r, 4), &object.name) {
            out.push(dep(
                DepDirection::UsedBy,
                DepVia::Trigger,
                kind,
                ns,
                name,
                None,
            ));
        } else if kind != DepKind::Table && mentions(&sql, &object.name) {
            out.push(dep(
                DepDirection::UsedBy,
                DepVia::Definition,
                kind,
                ns,
                name,
                None,
            ));
        }
    }
    if object.kind == DdlKind::View
        && let Some(own) = own
    {
        for r in rows {
            let kind = DepKind::parse(&cell(r, 1));
            let (ns, name) = (cell(r, 2), cell(r, 3));
            if cell(r, 0) == "object"
                && matches!(kind, Some(DepKind::Table | DepKind::View))
                && !is_it(&ns, &name)
                && mentions(&own, &name)
            {
                out.push(dep(
                    DepDirection::Uses,
                    DepVia::Definition,
                    kind.unwrap_or(DepKind::Table),
                    ns,
                    name,
                    None,
                ));
            }
        }
    }
    tidy(out)
}

pub fn views_from_engine(
    object: &ObjectRef,
    guessed: Vec<Dependency>,
    views: &[String],
    read: impl Fn(&str) -> Option<Vec<String>>,
) -> Vec<Dependency> {
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut out = guessed;
    if object.kind == DdlKind::View {
        if let Some(tables) = read(&object.name) {
            out.retain(|d| !(d.direction == DepDirection::Uses && d.via == DepVia::Definition));
            out.extend(tables.into_iter().map(|name| Dependency {
                direction: DepDirection::Uses,
                via: DepVia::Definition,
                kind: if views.iter().any(|v| same(v, &name)) {
                    DepKind::View
                } else {
                    DepKind::Table
                },
                namespace: object.namespace.clone(),
                name,
                detail: None,
            }));
        }
    } else {
        for view in views {
            let Some(tables) = read(view) else { continue };
            out.retain(|d| {
                !(d.direction == DepDirection::UsedBy
                    && d.kind == DepKind::View
                    && same(&d.name, view))
            });
            if tables.iter().any(|t| same(t, &object.name)) {
                out.push(Dependency {
                    direction: DepDirection::UsedBy,
                    via: DepVia::Definition,
                    kind: DepKind::View,
                    namespace: object.namespace.clone(),
                    name: view.clone(),
                    detail: None,
                });
            }
        }
    }
    tidy(out)
}
