use std::collections::BTreeMap;

use nd_driver::{
    ColumnInfo, FkAction, ForeignKey, Function, Index, Namespace, NamespaceSize, Object, Outline,
    Relation, RelationTag, Result, Schema,
};
use tokio_postgres::Client;

use crate::Flavor;
use crate::error::map_error;

const USER_NAMESPACES: &str = "n.nspname NOT IN ('pg_catalog', 'information_schema', 'crdb_internal', 'pg_extension') AND n.nspname NOT LIKE 'pg\\_toast%' AND n.nspname NOT LIKE 'pg\\_temp%'";

const RLS: &str = "SELECT n.nspname::text, c.relname::text, (SELECT count(*)::int4 FROM pg_policy p WHERE p.polrelid = c.oid)
     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relrowsecurity";

const SUPABASE: &[&str] = &[
    "auth",
    "storage",
    "realtime",
    "_realtime",
    "graphql",
    "graphql_public",
    "extensions",
    "vault",
    "pgsodium",
    "pgsodium_masks",
    "supabase_functions",
    "supabase_migrations",
    "net",
    "cron",
    "pgbouncer",
    "_analytics",
];

async fn tag_rls(
    client: &Client,
    scope: &str,
    namespaces: &mut BTreeMap<String, Namespace>,
) -> Result<()> {
    for r in query(client, format!("{RLS} AND {scope}")).await? {
        let ns: String = r.get(0);
        if let Some(n) = namespaces.get_mut(&ns) {
            let policies = u32::try_from(r.get::<_, i32>(2)).unwrap_or(0);
            n.tags
                .entry(r.get(1))
                .or_default()
                .push(RelationTag::Rls { policies });
        }
    }
    Ok(())
}

pub(crate) fn mark_supabase(schema: &mut Schema) {
    for n in &mut schema.namespaces {
        if SUPABASE.contains(&n.name.as_str()) {
            n.managed = Some(nd_driver::Flavor::Supabase);
        }
    }
}

#[derive(Clone, Copy)]
enum Extra {
    Sequences,
    Types,
    Triggers,
}

pub(crate) async fn outline(client: &Client, current: &str) -> Result<Outline> {
    let (databases, rows) = tokio::try_join!(
        load_databases(client),
        query(
            client,
            format!(
                "SELECT n.nspname, count(c.oid)::int8 FROM pg_namespace n
                 LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
                      AND NOT COALESCE(c.relispartition, false)
                 WHERE {USER_NAMESPACES} GROUP BY 1 ORDER BY n.nspname = 'public' DESC, 1"
            ),
        )
    )?;
    Ok(Outline {
        databases,
        current_database: current.to_owned(),
        namespaces: rows
            .iter()
            .map(|r| NamespaceSize {
                name: r.get(0),
                relations: u64::try_from(r.get::<_, i64>(1)).unwrap_or(0),
            })
            .collect(),
    })
}

async fn estimates_of(client: &Client, flavor: Flavor) -> Result<BTreeMap<(String, String), f64>> {
    match flavor {
        Flavor::Postgres => Ok(BTreeMap::new()),
        Flavor::Cockroach => cockroach_estimates(client).await,
    }
}

fn scope_of(only: Option<&[String]>) -> String {
    match only {
        Some(names) => {
            let list: Vec<String> = names
                .iter()
                .map(|n| nd_sql::literal(nd_sql::Dialect::Postgres, Some(n)))
                .collect();
            format!("{USER_NAMESPACES} AND n.nspname IN ({})", list.join(", "))
        }
        None => USER_NAMESPACES.to_owned(),
    }
}

pub(crate) async fn load(
    client: &Client,
    current: &str,
    flavor: Flavor,
    only: Option<&[String]>,
) -> Result<Schema> {
    let owned = scope_of(only);
    let scope = owned.as_str();
    let namespaces = query(
        client,
        format!(
            "SELECT n.nspname FROM pg_namespace n WHERE {scope} ORDER BY n.nspname = 'public' DESC, 1"
        ),
    );
    let relations = query(
        client,
        format!(
            "SELECT c.oid::int8, n.nspname, c.relname, c.relkind::text, c.reltuples::float8
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f') AND {scope} AND NOT COALESCE(c.relispartition, false)
             ORDER BY n.nspname, c.relname"
        ),
    );
    let estimates = estimates_of(client, flavor);
    let functions = query(
        client,
        format!(
            "SELECT n.nspname, p.proname, pg_get_function_identity_arguments(p.oid)
             FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
             WHERE p.prokind IN ('f', 'p') AND {scope} ORDER BY 2"
        ),
    );
    let (
        databases,
        namespaces,
        relations,
        mut estimates,
        mut columns,
        indexes,
        keys,
        functions,
        extras,
    ) = tokio::try_join!(
        load_databases(client),
        namespaces,
        relations,
        estimates,
        load_columns(client, scope),
        load_indexes(client, scope),
        load_foreign_keys(client, scope),
        functions,
        load_extras(client, scope, flavor),
    )?;
    let mut namespaces: BTreeMap<String, Namespace> = namespaces
        .iter()
        .map(|r| {
            let name: String = r.get(0);
            (name.clone(), Namespace::new(name))
        })
        .collect();
    let mut indexes = indexes.resolve(&mut columns);
    let mut foreign_keys = keys.resolve(client, &columns).await?;

    for r in &relations {
        let oid: i64 = r.get(0);
        let ns: String = r.get(1);
        let kind: String = r.get(3);
        let name: String = r.get(2);
        let estimate: Option<f64> = r.get(4);
        let estimate = estimate
            .filter(|e| *e >= 0.0)
            .or_else(|| estimates.remove(&(ns.clone(), name.clone())));
        let relation = Relation {
            name,
            columns: columns
                .remove(&oid)
                .map(|c| c.into_iter().map(|(_, c)| c).collect())
                .unwrap_or_default(),
            indexes: indexes.remove(&oid).unwrap_or_default(),
            foreign_keys: foreign_keys.remove(&oid).unwrap_or_default(),
            estimated_rows: estimate,
            detail: None,
        };
        if let Some(n) = namespaces.get_mut(&ns) {
            if matches!(kind.as_str(), "v" | "m") {
                n.views.push(relation);
            } else {
                n.tables.push(relation);
            }
        }
    }

    add_objects(&mut namespaces, &functions, extras);
    if flavor == Flavor::Postgres {
        tag_rls(client, scope, &mut namespaces).await?;
    }

    let mut ordered: Vec<Namespace> = namespaces.into_values().collect();
    ordered.sort_by_key(|n| n.name != "public");
    Ok(Schema {
        databases,
        current_database: current.to_owned(),
        namespaces: ordered,
    })
}

async fn load_databases(client: &Client) -> Result<Vec<String>> {
    Ok(query(
        client,
        "SELECT datname FROM pg_database WHERE datallowconn AND NOT datistemplate AND datname <> 'system' ORDER BY 1"
            .into(),
    )
    .await?
    .iter()
    .map(|r| r.get(0))
    .collect())
}

fn add_objects(
    namespaces: &mut BTreeMap<String, Namespace>,
    functions: &[tokio_postgres::Row],
    extras: Vec<(Extra, Vec<tokio_postgres::Row>)>,
) {
    for r in functions {
        let ns: String = r.get(0);
        if let Some(n) = namespaces.get_mut(&ns) {
            n.functions.push(Function {
                name: r.get(1),
                signature: r.get(2),
            });
        }
    }

    for (extra, rows) in extras {
        for r in rows {
            let ns: String = r.get(0);
            if let Some(n) = namespaces.get_mut(&ns) {
                let object = Object {
                    name: r.get(1),
                    detail: r.get(2),
                };
                match extra {
                    Extra::Sequences => n.sequences.push(object),
                    Extra::Types => n.types.push(object),
                    Extra::Triggers => n.triggers.push(object),
                }
            }
        }
    }
}

async fn load_extras(
    client: &Client,
    scope: &str,
    flavor: Flavor,
) -> Result<Vec<(Extra, Vec<tokio_postgres::Row>)>> {
    let extras = [
        (
            Extra::Sequences,
            format!(
                "SELECT n.nspname, c.relname::text, NULL::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
                 WHERE c.relkind = 'S' AND {scope} ORDER BY 2"
            ),
        ),
        (
            Extra::Types,
            format!(
                "SELECT n.nspname, t.typname::text, CASE t.typtype WHEN 'e' THEN 'enum' WHEN 'd' THEN 'domain' ELSE 'composite' END
                 FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace
                 WHERE (t.typtype IN ('e', 'd') OR (t.typtype = 'c' AND EXISTS (SELECT 1 FROM pg_class c WHERE c.oid = t.typrelid AND c.relkind = 'c')))
                   AND {scope} ORDER BY 2"
            ),
        ),
        (
            Extra::Triggers,
            format!(
                "SELECT n.nspname, g.tgname::text, c.relname::text FROM pg_trigger g
                 JOIN pg_class c ON c.oid = g.tgrelid JOIN pg_namespace n ON n.oid = c.relnamespace
                 WHERE NOT g.tgisinternal AND {scope} ORDER BY 2"
            ),
        ),
    ];
    futures_util::future::try_join_all(extras.into_iter().map(|(extra, sql)| async move {
        let found = match flavor {
            Flavor::Postgres => query(client, sql).await?,
            Flavor::Cockroach => query(client, sql).await.unwrap_or_default(),
        };
        Ok((extra, found))
    }))
    .await
}

#[allow(clippy::cast_precision_loss)]
async fn cockroach_estimates(client: &Client) -> Result<BTreeMap<(String, String), f64>> {
    Ok(query(
        client,
        "SELECT schema_name, table_name, estimated_row_count FROM [SHOW TABLES] WHERE type = 'table'"
            .into(),
    )
    .await?
    .iter()
    .filter_map(|r| {
        let rows: Option<i64> = r.get(2);
        Some(((r.get(0), r.get(1)), rows? as f64))
    })
    .collect())
}

type Columns = BTreeMap<i64, Vec<(i16, ColumnInfo)>>;

async fn load_columns(client: &Client, scope: &str) -> Result<Columns> {
    let mut columns: Columns = BTreeMap::new();
    for r in query(client, format!(
        "SELECT a.attrelid::int8, a.attnum, a.attname, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull
         FROM pg_attribute a
         JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE a.attnum > 0 AND NOT a.attisdropped AND c.relkind IN ('r', 'p', 'v', 'm', 'f') AND {scope}
           AND NOT COALESCE(c.relispartition, false)"
    ))
    .await?
    {
        columns.entry(r.get(0)).or_default().push((
            r.get(1),
            ColumnInfo {
                name: r.get(2),
                type_name: r.get(3),
                nullable: r.get(4),
                primary_key: false,
            },
        ));
    }
    for list in columns.values_mut() {
        list.sort_unstable_by_key(|(n, _)| *n);
    }
    Ok(columns)
}

fn column_name(columns: &Columns, table: i64, attnum: i16) -> Option<&str> {
    let list = columns.get(&table)?;
    let at = list.binary_search_by_key(&attnum, |(n, _)| *n).ok()?;
    Some(&list[at].1.name)
}

struct RawIndex {
    table: i64,
    index: Index,
    primary: bool,
    keys: Vec<i16>,
}

struct RawIndexes(Vec<RawIndex>);

impl RawIndexes {
    fn resolve(self, columns: &mut Columns) -> BTreeMap<i64, Vec<Index>> {
        let mut out: BTreeMap<i64, Vec<Index>> = BTreeMap::new();
        for raw in self.0 {
            let mut index = raw.index;
            if index.columns.is_empty() {
                index.columns = raw
                    .keys
                    .iter()
                    .filter_map(|k| column_name(columns, raw.table, *k).map(str::to_owned))
                    .collect();
            }
            if raw.primary
                && let Some(list) = columns.get_mut(&raw.table)
            {
                for (n, c) in list.iter_mut() {
                    c.primary_key |= raw.keys.contains(n);
                }
            }
            out.entry(raw.table).or_default().push(index);
        }
        out
    }
}

async fn load_indexes(client: &Client, scope: &str) -> Result<RawIndexes> {
    let rows = query(client, format!(
        "SELECT i.indrelid::int8, ic.relname, i.indisunique, i.indisprimary, i.indkey::int2[], i.indnkeyatts::int4,
                CASE WHEN i.indexprs IS NULL THEN NULL
                     ELSE ARRAY(SELECT pg_get_indexdef(i.indexrelid, k, true) FROM generate_series(1, i.indnkeyatts) k) END
         FROM pg_index i JOIN pg_class ic ON ic.oid = i.indexrelid
         JOIN pg_namespace n ON n.oid = ic.relnamespace
         WHERE {scope} ORDER BY ic.relname"
    ))
    .await?;
    Ok(RawIndexes(
        rows.iter()
            .map(|r| {
                let mut keys: Vec<i16> = r.get(4);
                keys.truncate(usize::try_from(r.get::<_, i32>(5)).unwrap_or(0));
                let text: Option<Vec<Option<String>>> = r.get(6);
                RawIndex {
                    table: r.get(0),
                    index: Index {
                        name: r.get(1),
                        unique: r.get(2),
                        columns: text.unwrap_or_default().into_iter().flatten().collect(),
                    },
                    primary: r.get(3),
                    keys,
                }
            })
            .collect(),
    ))
}

struct RawKey {
    table: i64,
    target: i64,
    from: Vec<i16>,
    to: Vec<i16>,
    key: ForeignKey,
}

struct RawKeys(Vec<RawKey>);

impl RawKeys {
    async fn resolve(
        self,
        client: &Client,
        columns: &Columns,
    ) -> Result<BTreeMap<i64, Vec<ForeignKey>>> {
        let mut outside: Vec<i64> = self
            .0
            .iter()
            .filter(|k| columns.contains_key(&k.table) && !columns.contains_key(&k.target))
            .map(|k| k.target)
            .collect();
        outside.sort_unstable();
        outside.dedup();
        let mut extra: Columns = BTreeMap::new();
        if !outside.is_empty() {
            for r in client
                .query(
                    "SELECT attrelid::int8, attnum, attname::text FROM pg_attribute WHERE attrelid::int8 = ANY($1) AND attnum > 0",
                    &[&outside],
                )
                .await
                .map_err(|e| map_error(&e, "pg_attribute"))?
            {
                extra.entry(r.get(0)).or_default().push((
                    r.get(1),
                    ColumnInfo {
                        name: r.get(2),
                        type_name: String::new(),
                        nullable: true,
                        primary_key: false,
                    },
                ));
            }
            for list in extra.values_mut() {
                list.sort_unstable_by_key(|(n, _)| *n);
            }
        }
        let name = |table: i64, n: i16| {
            column_name(columns, table, n)
                .or_else(|| column_name(&extra, table, n))
                .unwrap_or_default()
                .to_owned()
        };
        let mut out: BTreeMap<i64, Vec<ForeignKey>> = BTreeMap::new();
        for raw in self.0 {
            let mut key = raw.key;
            key.columns = raw.from.iter().map(|n| name(raw.table, *n)).collect();
            key.ref_columns = raw.to.iter().map(|n| name(raw.target, *n)).collect();
            out.entry(raw.table).or_default().push(key);
        }
        Ok(out)
    }
}

async fn load_foreign_keys(client: &Client, scope: &str) -> Result<RawKeys> {
    let rows = query(client, format!(
        "SELECT c.conrelid::int8, c.conname::text, rn.nspname::text, rc.relname::text,
                c.conkey, c.confrelid::int8, c.confkey, c.confdeltype::text, c.confupdtype::text
         FROM pg_constraint c
         JOIN pg_namespace n ON n.oid = c.connamespace
         JOIN pg_class rc ON rc.oid = c.confrelid JOIN pg_namespace rn ON rn.oid = rc.relnamespace
         JOIN pg_class tc ON tc.oid = c.conrelid
         WHERE c.contype = 'f' AND {scope} AND NOT COALESCE(tc.relispartition, false) ORDER BY c.conname"
    ))
    .await?;
    Ok(RawKeys(
        rows.iter()
            .map(|r| {
                let on_delete: String = r.get(7);
                let on_update: String = r.get(8);
                RawKey {
                    table: r.get(0),
                    target: r.get(5),
                    from: r.get(4),
                    to: r.get(6),
                    key: ForeignKey {
                        name: r.get(1),
                        ref_namespace: r.get(2),
                        ref_table: r.get(3),
                        columns: Vec::new(),
                        ref_columns: Vec::new(),
                        on_delete: FkAction::parse(&on_delete),
                        on_update: FkAction::parse(&on_update),
                    },
                }
            })
            .collect(),
    ))
}

async fn query(client: &Client, sql: String) -> Result<Vec<tokio_postgres::Row>> {
    client
        .query(&sql, &[])
        .await
        .map_err(|e| map_error(&e, &sql))
}
