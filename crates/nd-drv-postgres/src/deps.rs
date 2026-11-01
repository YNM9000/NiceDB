use nd_driver::{Dependency, Dialect, ObjectRef, Result, ident, tidy};
use tokio_postgres::Client;

use crate::error::map_error;

const QUERY: &str = "WITH o AS (SELECT to_regclass($1::text)::oid AS id),
     rel AS (SELECT c.oid, n.nspname, c.relname,
                    CASE WHEN c.relkind IN ('v', 'm') THEN 'view' WHEN c.relkind = 'S' THEN 'sequence' ELSE 'table' END AS kind
               FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace)
SELECT 'uses', 'definition', rel.kind, rel.nspname, rel.relname, NULL::text
  FROM pg_depend d JOIN pg_rewrite r ON r.oid = d.objid JOIN rel ON rel.oid = d.refobjid, o
 WHERE d.classid = 'pg_rewrite'::regclass AND d.refclassid = 'pg_class'::regclass AND r.ev_class = o.id AND rel.oid <> o.id
UNION
SELECT 'uses', 'definition', 'routine', n.nspname, p.proname, NULL
  FROM pg_depend d JOIN pg_rewrite r ON r.oid = d.objid JOIN pg_proc p ON p.oid = d.refobjid
  JOIN pg_namespace n ON n.oid = p.pronamespace, o
 WHERE d.classid = 'pg_rewrite'::regclass AND d.refclassid = 'pg_proc'::regclass AND r.ev_class = o.id
   AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'crdb_internal')
UNION
SELECT 'used_by', 'definition', rel.kind, rel.nspname, rel.relname, NULL
  FROM pg_depend d JOIN pg_rewrite r ON r.oid = d.objid JOIN rel ON rel.oid = r.ev_class, o
 WHERE d.classid = 'pg_rewrite'::regclass AND d.refobjid = o.id AND rel.oid <> o.id
UNION
SELECT 'uses', 'foreign_key', 'table', rel.nspname, rel.relname, k.conname::text
  FROM pg_constraint k JOIN rel ON rel.oid = k.confrelid, o
 WHERE k.contype = 'f' AND k.conrelid = o.id
UNION
SELECT 'used_by', 'foreign_key', 'table', rel.nspname, rel.relname, k.conname::text
  FROM pg_constraint k JOIN rel ON rel.oid = k.conrelid, o
 WHERE k.contype = 'f' AND k.confrelid = o.id
UNION
SELECT 'used_by', 'trigger', 'trigger', rel.nspname, t.tgname::text, p.proname::text
  FROM pg_trigger t JOIN rel ON rel.oid = t.tgrelid LEFT JOIN pg_proc p ON p.oid = t.tgfoid, o
 WHERE t.tgrelid = o.id AND NOT t.tgisinternal
UNION
SELECT 'uses', 'owned', 'sequence', rel.nspname, rel.relname, NULL
  FROM pg_depend d JOIN rel ON rel.oid = d.objid, o
 WHERE d.classid = 'pg_class'::regclass AND d.refobjid = o.id AND rel.kind = 'sequence'";

pub(crate) async fn dependencies(client: &Client, object: &ObjectRef) -> Result<Vec<Dependency>> {
    let qualified = format!(
        "{}.{}",
        ident(Dialect::Postgres, &object.namespace),
        ident(Dialect::Postgres, &object.name)
    );
    let rows = client
        .query(QUERY, &[&qualified])
        .await
        .map_err(|e| map_error(&e, QUERY))?;
    Ok(tidy(
        rows.iter()
            .filter_map(|r| {
                let row: Vec<Option<String>> = (0..6).map(|i| r.get(i)).collect();
                Dependency::from_row(&row)
            })
            .collect(),
    ))
}
