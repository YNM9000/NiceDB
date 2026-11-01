use nd_driver::{Dependency, ObjectRef, Result, tidy};

use crate::Tds;
use crate::ddl::rows;

const QUERY: &str = "WITH k AS (SELECT o.object_id AS id FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id
                    WHERE s.name = @P1 AND o.name = @P2 AND o.type IN ('U', 'V')),
     obj AS (SELECT o.object_id, SCHEMA_NAME(o.schema_id) AS ns, o.name,
                    CASE WHEN o.type = 'U' THEN 'table' WHEN o.type = 'V' THEN 'view' WHEN o.type = 'TR' THEN 'trigger' WHEN o.type = 'SO' THEN 'sequence' WHEN o.type IN ('P', 'FN', 'IF', 'TF', 'PC', 'FS', 'FT') THEN 'routine' END AS kind
               FROM sys.objects o)
SELECT 'uses' AS dir, 'definition' AS via, obj.kind, obj.ns, obj.name, CAST(NULL AS nvarchar(128)) AS detail
  FROM sys.sql_expression_dependencies d JOIN k ON d.referencing_id = k.id JOIN obj ON obj.object_id = d.referenced_id
 WHERE d.referenced_id <> k.id
UNION
SELECT 'used_by', CASE WHEN obj.kind = 'trigger' THEN 'trigger' ELSE 'definition' END, obj.kind, obj.ns, obj.name, NULL
  FROM sys.sql_expression_dependencies d JOIN k ON d.referenced_id = k.id JOIN obj ON obj.object_id = d.referencing_id
 WHERE d.referencing_id <> k.id
UNION
SELECT 'uses', 'foreign_key', 'table', obj.ns, obj.name, f.name
  FROM sys.foreign_keys f JOIN k ON f.parent_object_id = k.id JOIN obj ON obj.object_id = f.referenced_object_id
UNION
SELECT 'used_by', 'foreign_key', 'table', obj.ns, obj.name, f.name
  FROM sys.foreign_keys f JOIN k ON f.referenced_object_id = k.id JOIN obj ON obj.object_id = f.parent_object_id
UNION
SELECT 'used_by', 'trigger', 'trigger', obj.ns, obj.name, NULL
  FROM sys.triggers t JOIN k ON t.parent_id = k.id JOIN obj ON obj.object_id = t.object_id";

pub(crate) async fn dependencies(client: &mut Tds, object: &ObjectRef) -> Result<Vec<Dependency>> {
    let found = rows(client, QUERY, &[&object.namespace, &object.name]).await?;
    Ok(tidy(
        found
            .iter()
            .filter_map(|r| Dependency::from_row(r))
            .collect(),
    ))
}
