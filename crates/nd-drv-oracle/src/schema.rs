use std::collections::BTreeMap;

use nd_driver::{ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, Object, Relation};

pub(crate) type Rows = Vec<Vec<Option<String>>>;

const OWNERS: &str =
    "IN (SELECT username FROM all_users WHERE oracle_maintained = 'N' OR username = USER)";

fn owned(sql: &str, column: &str) -> String {
    sql.replace("{owner}", &format!("{column} {OWNERS}"))
}

#[allow(clippy::too_many_lines)]
pub(crate) fn queries() -> Vec<(Part, String)> {
    vec![
        (
            Part::Schemas,
            "SELECT username FROM all_users WHERE (oracle_maintained = 'N' OR username = USER) \
             AND (username = USER OR EXISTS (SELECT 1 FROM all_objects o WHERE o.owner = username)) \
             ORDER BY CASE WHEN username = USER THEN 0 ELSE 1 END, username"
                .to_owned(),
        ),
        (
            Part::Tables,
            owned(
                "SELECT t.owner, t.table_name, t.num_rows FROM all_tables t WHERE t.{owner} \
                 AND t.nested = 'NO' AND t.secondary = 'N' AND t.dropped = 'NO' \
                 AND (t.iot_type IS NULL OR t.iot_type = 'IOT') \
                 AND NOT EXISTS (SELECT 1 FROM all_mviews m WHERE m.owner = t.owner AND m.mview_name = t.table_name) \
                 ORDER BY t.owner, t.table_name",
                "owner",
            ),
        ),
        (
            Part::Views,
            owned(
                "SELECT owner, view_name FROM all_views WHERE {owner} ORDER BY owner, view_name",
                "owner",
            ),
        ),
        (
            Part::Mviews,
            owned(
                "SELECT owner, mview_name FROM all_mviews WHERE {owner} ORDER BY owner, mview_name",
                "owner",
            ),
        ),
        (
            Part::Columns,
            owned(
                "SELECT owner, table_name, column_name, data_type, data_length, char_length, \
                 char_used, data_precision, data_scale, nullable FROM all_tab_columns WHERE {owner} \
                 ORDER BY owner, table_name, column_id",
                "owner",
            ),
        ),
        (
            Part::PrimaryKeys,
            owned(
                "SELECT c.owner, c.table_name, k.column_name FROM all_constraints c \
                 JOIN all_cons_columns k ON k.owner = c.owner AND k.constraint_name = c.constraint_name \
                 WHERE c.constraint_type = 'P' AND c.{owner} ORDER BY c.owner, c.table_name, k.position",
                "owner",
            ),
        ),
        (
            Part::Indexes,
            owned(
                "SELECT i.table_owner, i.table_name, i.index_name, i.uniqueness, c.column_name \
                 FROM all_indexes i JOIN all_ind_columns c ON c.index_owner = i.owner AND c.index_name = i.index_name \
                 WHERE i.{owner} AND i.index_type <> 'LOB' \
                 ORDER BY i.table_owner, i.table_name, i.index_name, c.column_position",
                "table_owner",
            ),
        ),
        (
            Part::ForeignKeys,
            owned(
                "SELECT c.owner, c.table_name, c.constraint_name, r.owner, r.table_name, c.delete_rule, \
                 k.column_name, rk.column_name FROM all_constraints c \
                 JOIN all_constraints r ON r.owner = c.r_owner AND r.constraint_name = c.r_constraint_name \
                 JOIN all_cons_columns k ON k.owner = c.owner AND k.constraint_name = c.constraint_name \
                 JOIN all_cons_columns rk ON rk.owner = r.owner AND rk.constraint_name = r.constraint_name AND rk.position = k.position \
                 WHERE c.constraint_type = 'R' AND c.{owner} \
                 ORDER BY c.owner, c.table_name, c.constraint_name, k.position",
                "owner",
            ),
        ),
        (
            Part::Routines,
            owned(
                "SELECT o.owner, o.object_name, o.object_type, \
                 (SELECT LISTAGG(a.argument_name || ' ' || DECODE(a.in_out, 'IN', '', a.in_out || ' ') || a.data_type, ', ') \
                    WITHIN GROUP (ORDER BY a.position) FROM all_arguments a \
                    WHERE a.owner = o.owner AND a.object_name = o.object_name AND a.package_name IS NULL \
                    AND a.position > 0 AND a.data_level = 0) \
                 FROM all_objects o WHERE o.object_type IN ('PROCEDURE', 'FUNCTION') AND o.{owner} \
                 ORDER BY o.owner, o.object_name",
                "owner",
            ),
        ),
        (
            Part::Packages,
            owned(
                "SELECT o.owner, o.object_name, (SELECT 'body' FROM all_objects b WHERE b.owner = o.owner \
                 AND b.object_name = o.object_name AND b.object_type = 'PACKAGE BODY') \
                 FROM all_objects o WHERE o.object_type = 'PACKAGE' AND o.{owner} ORDER BY o.owner, o.object_name",
                "owner",
            ),
        ),
        (
            Part::Sequences,
            owned(
                "SELECT sequence_owner, sequence_name FROM all_sequences WHERE {owner} \
                 ORDER BY sequence_owner, sequence_name",
                "sequence_owner",
            ),
        ),
        (
            Part::Synonyms,
            owned(
                "SELECT owner, synonym_name, table_owner || '.' || table_name || NVL2(db_link, '@' || db_link, '') \
                 FROM all_synonyms WHERE {owner} ORDER BY owner, synonym_name",
                "owner",
            ),
        ),
        (
            Part::Triggers,
            owned(
                "SELECT owner, trigger_name, table_name FROM all_triggers WHERE {owner} \
                 ORDER BY owner, trigger_name",
                "owner",
            ),
        ),
        (
            Part::Types,
            owned(
                "SELECT owner, type_name, typecode FROM all_types WHERE {owner} ORDER BY owner, type_name",
                "owner",
            ),
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Part {
    Schemas,
    Tables,
    Views,
    Mviews,
    Columns,
    PrimaryKeys,
    Indexes,
    ForeignKeys,
    Routines,
    Packages,
    Sequences,
    Synonyms,
    Triggers,
    Types,
}

fn s(r: &[Option<String>], i: usize) -> String {
    r.get(i).cloned().flatten().unwrap_or_default()
}

fn n(r: &[Option<String>], i: usize) -> Option<u64> {
    r.get(i).cloned().flatten().and_then(|v| v.parse().ok())
}

pub(crate) fn column_type(
    data_type: &str,
    length: Option<u64>,
    chars: Option<u64>,
    char_used: &str,
    precision: Option<u64>,
    scale: Option<u64>,
) -> String {
    match data_type {
        "VARCHAR2" | "CHAR" if char_used == "C" => {
            format!("{data_type}({} CHAR)", chars.unwrap_or_default())
        }
        "VARCHAR2" | "CHAR" | "RAW" => format!("{data_type}({})", length.unwrap_or_default()),
        "NVARCHAR2" | "NCHAR" => format!("{data_type}({})", chars.unwrap_or_default()),
        "NUMBER" => match (precision, scale) {
            (None, None) => "NUMBER".to_owned(),
            (None, Some(0)) => "INTEGER".to_owned(),
            (None, Some(s)) => format!("NUMBER(*,{s})"),
            (Some(p), None | Some(0)) => format!("NUMBER({p})"),
            (Some(p), Some(s)) => format!("NUMBER({p},{s})"),
        },
        "FLOAT" => precision.map_or_else(|| "FLOAT".to_owned(), |p| format!("FLOAT({p})")),
        other => other.to_owned(),
    }
}

fn rel_at(spaces: &mut [Namespace], ni: usize, kind: u8, ri: usize) -> &mut Relation {
    let ns = &mut spaces[ni];
    match kind {
        0 => &mut ns.tables[ri],
        1 => &mut ns.views[ri],
        _ => &mut ns.materialized_views[ri],
    }
}

fn relation(name: String, estimated_rows: Option<f64>) -> Relation {
    Relation {
        name,
        columns: Vec::new(),
        indexes: Vec::new(),
        foreign_keys: Vec::new(),
        estimated_rows,
        detail: None,
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn assemble(parts: &BTreeMap<Part, Rows>) -> Vec<Namespace> {
    let empty = Rows::new();
    let get = |p: Part| parts.get(&p).unwrap_or(&empty);
    let mut spaces: Vec<Namespace> = get(Part::Schemas)
        .iter()
        .map(|r| Namespace::new(s(r, 0)))
        .collect();
    let at = |spaces: &[Namespace], owner: &str| spaces.iter().position(|n| n.name == owner);
    #[allow(clippy::cast_precision_loss)]
    for r in get(Part::Tables) {
        if let Some(i) = at(&spaces, &s(r, 0)) {
            spaces[i]
                .tables
                .push(relation(s(r, 1), n(r, 2).map(|v| v as f64)));
        }
    }
    for r in get(Part::Views) {
        if let Some(i) = at(&spaces, &s(r, 0)) {
            spaces[i].views.push(relation(s(r, 1), None));
        }
    }
    for r in get(Part::Mviews) {
        if let Some(i) = at(&spaces, &s(r, 0)) {
            spaces[i].materialized_views.push(relation(s(r, 1), None));
        }
    }
    let mut keys: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for r in get(Part::PrimaryKeys) {
        keys.entry((s(r, 0), s(r, 1))).or_default().push(s(r, 2));
    }
    {
        let mut index: BTreeMap<(String, String), (usize, u8, usize)> = BTreeMap::new();
        for (ni, ns) in spaces.iter().enumerate() {
            for (kind, list) in [&ns.tables, &ns.views, &ns.materialized_views]
                .into_iter()
                .enumerate()
            {
                for (ri, rel) in list.iter().enumerate() {
                    index.insert(
                        (ns.name.clone(), rel.name.clone()),
                        (ni, u8::try_from(kind).unwrap_or(0), ri),
                    );
                }
            }
        }
        for r in get(Part::Columns) {
            let (owner, table) = (s(r, 0), s(r, 1));
            let Some(&(ni, kind, ri)) = index.get(&(owner.clone(), table.clone())) else {
                continue;
            };
            let name = s(r, 2);
            let primary_key = keys.get(&(owner, table)).is_some_and(|k| k.contains(&name));
            let rel = rel_at(&mut spaces, ni, kind, ri);
            rel.columns.push(ColumnInfo {
                name,
                type_name: column_type(&s(r, 3), n(r, 4), n(r, 5), &s(r, 6), n(r, 7), n(r, 8)),
                nullable: s(r, 9) != "N",
                primary_key,
            });
        }
        for r in get(Part::Indexes) {
            let Some(&(ni, kind, ri)) = index.get(&(s(r, 0), s(r, 1))) else {
                continue;
            };
            let rel = rel_at(&mut spaces, ni, kind, ri);
            let name = s(r, 2);
            match rel.indexes.iter_mut().find(|i| i.name == name) {
                Some(i) => i.columns.push(s(r, 4)),
                None => rel.indexes.push(Index {
                    name,
                    columns: vec![s(r, 4)],
                    unique: s(r, 3) == "UNIQUE",
                }),
            }
        }
        for r in get(Part::ForeignKeys) {
            let Some(&(ni, kind, ri)) = index.get(&(s(r, 0), s(r, 1))) else {
                continue;
            };
            let rel = rel_at(&mut spaces, ni, kind, ri);
            let name = s(r, 2);
            if !rel.foreign_keys.iter().any(|f| f.name == name) {
                rel.foreign_keys.push(ForeignKey {
                    name: name.clone(),
                    columns: Vec::new(),
                    ref_namespace: s(r, 3),
                    ref_table: s(r, 4),
                    ref_columns: Vec::new(),
                    on_delete: FkAction::parse(&s(r, 5)),
                    on_update: FkAction::NoAction,
                });
            }
            let Some(fk) = rel.foreign_keys.iter_mut().find(|f| f.name == name) else {
                continue;
            };
            fk.columns.push(s(r, 6));
            fk.ref_columns.push(s(r, 7));
        }
    }
    for r in get(Part::Routines) {
        if let Some(i) = at(&spaces, &s(r, 0)) {
            spaces[i].functions.push(Function {
                name: s(r, 1),
                signature: s(r, 3),
            });
        }
    }
    let objects =
        |spaces: &mut Vec<Namespace>, part: Part, pick: fn(&mut Namespace) -> &mut Vec<Object>| {
            for r in get(part) {
                if let Some(i) = at(spaces, &s(r, 0)) {
                    pick(&mut spaces[i]).push(Object {
                        name: s(r, 1),
                        detail: r.get(2).cloned().flatten(),
                    });
                }
            }
        };
    objects(&mut spaces, Part::Packages, |n| &mut n.packages);
    objects(&mut spaces, Part::Sequences, |n| &mut n.sequences);
    objects(&mut spaces, Part::Synonyms, |n| &mut n.synonyms);
    objects(&mut spaces, Part::Triggers, |n| &mut n.triggers);
    objects(&mut spaces, Part::Types, |n| &mut n.types);
    spaces
}
