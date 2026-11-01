use std::collections::{BTreeMap, HashMap};

use futures_util::TryStreamExt;
use nd_driver::{
    ColumnInfo, DriverError, FkAction, ForeignKey, Function, Index, Namespace, NamespaceSize,
    Object, Outline, Relation, Result, Schema,
};
use tiberius::{FromSql, Query, QueryItem, Row};

use crate::Tds;
use crate::error::map_error;

const ROUTINES: &str = "('P', 'PC', 'FN', 'IF', 'TF', 'FS', 'FT')";

const HEAD: &str = "SET NOCOUNT ON;
SELECT name FROM sys.databases WHERE HAS_DBACCESS(name) = 1 ORDER BY name;
SELECT DB_NAME();
SELECT s.name FROM sys.schemas s WHERE s.schema_id < 16384 AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest') SCOPE;";

const RELATIONS: &str = "
SELECT o.object_id, s.name, o.name, CASE WHEN o.type = 'V' THEN 1 ELSE 0 END, p.n
  FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id
  LEFT JOIN (SELECT object_id, SUM(rows) AS n FROM sys.partitions WHERE index_id IN (0, 1) GROUP BY object_id) p
         ON p.object_id = o.object_id
 WHERE o.is_ms_shipped = 0 AND o.type IN ('U', 'V') OBJECT ORDER BY s.name, o.name;
SELECT c.object_id, c.column_id, c.name, t.name, c.max_length, c.precision, c.scale, c.is_nullable
  FROM sys.columns c JOIN sys.objects o ON o.object_id = c.object_id
  JOIN sys.types t ON t.user_type_id = c.user_type_id
 WHERE o.is_ms_shipped = 0 AND o.type IN ('U', 'V') OBJECT ORDER BY c.object_id, c.column_id;
SELECT i.object_id, i.name, i.is_unique, i.is_primary_key, ic.column_id
  FROM sys.indexes i JOIN sys.objects o ON o.object_id = i.object_id
  JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.is_included_column = 0
 WHERE i.name IS NOT NULL AND o.is_ms_shipped = 0 AND o.type IN ('U', 'V') OBJECT
 ORDER BY i.object_id, i.name, ic.key_ordinal;
SELECT fk.parent_object_id, fk.name, rs.name, ro.name, fkc.parent_column_id, rc.name,
       fk.delete_referential_action_desc, fk.update_referential_action_desc
  FROM sys.foreign_keys fk JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id
  JOIN sys.objects o ON o.object_id = fk.parent_object_id
  JOIN sys.objects ro ON ro.object_id = fk.referenced_object_id
  JOIN sys.schemas rs ON rs.schema_id = ro.schema_id
  JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id
 WHERE 1 = 1 OBJECT
 ORDER BY fk.parent_object_id, fk.name, fkc.constraint_column_id;";

const TAIL: &str = "
SELECT o.object_id, s.name, o.name FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id
 WHERE o.is_ms_shipped = 0 AND o.type IN ROUTINES SCOPE ORDER BY o.name;
SELECT p.object_id, p.name, TYPE_NAME(p.user_type_id), p.is_output
  FROM sys.parameters p JOIN sys.objects o ON o.object_id = p.object_id
 WHERE p.parameter_id > 0 AND o.is_ms_shipped = 0 AND o.type IN ROUTINES
   AND SCHEMA_NAME(o.schema_id) IN (SELECT name FROM sys.schemas s WHERE 1 = 1 SCOPE) ORDER BY p.object_id, p.parameter_id;
SELECT s.name, q.name, CAST(NULL AS nvarchar(1)) FROM sys.sequences q JOIN sys.schemas s ON s.schema_id = q.schema_id
 WHERE 1 = 1 SCOPE ORDER BY q.name;
SELECT s.name, t.name, CASE WHEN t.is_table_type = 1 THEN 'table' ELSE TYPE_NAME(t.system_type_id) END
  FROM sys.types t JOIN sys.schemas s ON s.schema_id = t.schema_id WHERE t.is_user_defined = 1 SCOPE ORDER BY t.name;
SELECT s.name, tr.name, o.name FROM sys.triggers tr JOIN sys.objects o ON o.object_id = tr.parent_id
  JOIN sys.schemas s ON s.schema_id = o.schema_id
 WHERE tr.parent_class = 1 AND tr.is_ms_shipped = 0 SCOPE ORDER BY tr.name;";

const OUTLINE: &str = "SET NOCOUNT ON;
SELECT name FROM sys.databases WHERE HAS_DBACCESS(name) = 1 ORDER BY name;
SELECT DB_NAME();
SELECT s.name, COUNT(o.object_id) FROM sys.schemas s
  LEFT JOIN sys.objects o ON o.schema_id = s.schema_id AND o.is_ms_shipped = 0 AND o.type IN ('U', 'V')
 WHERE s.schema_id < 16384 AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')
 GROUP BY s.name ORDER BY CASE WHEN s.name = 'dbo' THEN 0 ELSE 1 END, s.name;";

fn get<'a, T: FromSql<'a>>(row: &'a Row, i: usize) -> Result<Option<T>> {
    row.try_get(i)
        .map_err(|e| DriverError::query(format!("catalog column {i}: {e}")))
}

fn text(row: &Row, i: usize) -> Result<String> {
    Ok(get::<&str>(row, i)?.unwrap_or_default().to_owned())
}

fn id(row: &Row, i: usize) -> Result<i32> {
    Ok(get::<i32>(row, i)?.unwrap_or_default())
}

fn bit(row: &Row, i: usize) -> Result<bool> {
    Ok(get::<bool>(row, i)?.unwrap_or_default())
}

fn type_name(name: &str, max_length: i16, precision: u8, scale: u8) -> String {
    let length = |unit: i16| {
        if max_length == -1 {
            "max".to_owned()
        } else {
            (max_length / unit).to_string()
        }
    };
    match name {
        "varchar" | "char" | "varbinary" | "binary" => format!("{name}({})", length(1)),
        "nvarchar" | "nchar" => format!("{name}({})", length(2)),
        "decimal" | "numeric" => format!("{name}({precision},{scale})"),
        "datetime2" | "time" | "datetimeoffset" => format!("{name}({scale})"),
        _ => name.to_owned(),
    }
}

struct Columns {
    by_object: HashMap<i32, Vec<(i32, ColumnInfo)>>,
}

impl Columns {
    fn read(rows: &[Row]) -> Result<Self> {
        let mut by_object: HashMap<i32, Vec<(i32, ColumnInfo)>> = HashMap::new();
        for r in rows {
            let info = ColumnInfo {
                name: text(r, 2)?,
                type_name: type_name(
                    get::<&str>(r, 3)?.unwrap_or_default(),
                    get(r, 4)?.unwrap_or_default(),
                    get(r, 5)?.unwrap_or_default(),
                    get(r, 6)?.unwrap_or_default(),
                ),
                nullable: bit(r, 7)?,
                primary_key: false,
            };
            by_object
                .entry(id(r, 0)?)
                .or_default()
                .push((id(r, 1)?, info));
        }
        Ok(Self { by_object })
    }

    fn get_mut(&mut self, object: i32, column: i32) -> Option<&mut ColumnInfo> {
        let list = self.by_object.get_mut(&object)?;
        let at = list.binary_search_by_key(&column, |(c, _)| *c).ok()?;
        list.get_mut(at).map(|(_, info)| info)
    }

    fn name(&self, object: i32, column: i32) -> String {
        self.by_object
            .get(&object)
            .and_then(|list| {
                let at = list.binary_search_by_key(&column, |(c, _)| *c).ok()?;
                list.get(at)
            })
            .map(|(_, info)| info.name.clone())
            .unwrap_or_default()
    }

    fn take(&mut self, object: i32) -> Vec<ColumnInfo> {
        self.by_object
            .remove(&object)
            .map(|list| list.into_iter().map(|(_, info)| info).collect())
            .unwrap_or_default()
    }
}

fn indexes(rows: &[Row], columns: &mut Columns) -> Result<HashMap<i32, Vec<Index>>> {
    let mut out: HashMap<i32, Vec<Index>> = HashMap::new();
    for r in rows {
        let (object, column) = (id(r, 0)?, id(r, 4)?);
        if bit(r, 3)?
            && let Some(c) = columns.get_mut(object, column)
        {
            c.primary_key = true;
        }
        let list = out.entry(object).or_default();
        let name = get::<&str>(r, 1)?.unwrap_or_default();
        if list.last().is_none_or(|i| i.name != name) {
            list.push(Index {
                name: name.to_owned(),
                columns: Vec::new(),
                unique: bit(r, 2)?,
            });
        }
        if let Some(i) = list.last_mut() {
            i.columns.push(columns.name(object, column));
        }
    }
    Ok(out)
}

fn foreign_keys(rows: &[Row], columns: &Columns) -> Result<HashMap<i32, Vec<ForeignKey>>> {
    let action = |r: &Row, i: usize| -> Result<FkAction> {
        Ok(FkAction::parse(
            &get::<&str>(r, i)?.unwrap_or_default().replace('_', " "),
        ))
    };
    let mut out: HashMap<i32, Vec<ForeignKey>> = HashMap::new();
    for r in rows {
        let parent = id(r, 0)?;
        let list = out.entry(parent).or_default();
        let name = get::<&str>(r, 1)?.unwrap_or_default();
        if list.last().is_none_or(|k| k.name != name) {
            list.push(ForeignKey {
                name: name.to_owned(),
                columns: Vec::new(),
                ref_namespace: text(r, 2)?,
                ref_table: text(r, 3)?,
                ref_columns: Vec::new(),
                on_delete: action(r, 6)?,
                on_update: action(r, 7)?,
            });
        }
        if let Some(k) = list.last_mut() {
            k.columns.push(columns.name(parent, id(r, 4)?));
            k.ref_columns.push(text(r, 5)?);
        }
    }
    Ok(out)
}

fn signatures(rows: &[Row]) -> Result<HashMap<i32, String>> {
    let mut out: HashMap<i32, String> = HashMap::new();
    for r in rows {
        let s = out.entry(id(r, 0)?).or_default();
        if !s.is_empty() {
            s.push_str(", ");
        }
        s.push_str(get::<&str>(r, 1)?.unwrap_or_default());
        s.push(' ');
        s.push_str(get::<&str>(r, 2)?.unwrap_or_default());
        if bit(r, 3)? {
            s.push_str(" OUTPUT");
        }
    }
    Ok(out)
}

fn objects(
    rows: &[Row],
    namespaces: &mut BTreeMap<String, Namespace>,
    pick: fn(&mut Namespace) -> &mut Vec<Object>,
) -> Result<()> {
    for r in rows {
        if let Some(n) = namespaces.get_mut(get::<&str>(r, 0)?.unwrap_or_default()) {
            pick(n).push(Object {
                name: text(r, 1)?,
                detail: get::<&str>(r, 2)?.map(str::to_owned),
            });
        }
    }
    Ok(())
}

async fn result_sets(client: &mut Tds, sql: &str, params: &[&str]) -> Result<Vec<Vec<Row>>> {
    let run = async {
        let mut sets: Vec<Vec<Row>> = Vec::new();
        let mut stream = if params.is_empty() {
            client.simple_query(sql).await?
        } else {
            let mut q = Query::new(sql);
            for p in params {
                q.bind(*p);
            }
            q.query(client).await?
        };
        while let Some(item) = stream.try_next().await? {
            match item {
                QueryItem::Metadata(_) => sets.push(Vec::new()),
                QueryItem::Row(row) => {
                    if let Some(set) = sets.last_mut() {
                        set.push(row);
                    }
                }
            }
        }
        Ok::<_, tiberius::error::Error>(sets)
    };
    run.await.map_err(|e| map_error(&e, sql))
}

fn sets<const N: usize>(sets: Vec<Vec<Row>>) -> Result<[Vec<Row>; N]> {
    <[Vec<Row>; N]>::try_from(sets).map_err(|sets| {
        DriverError::query(format!(
            "the catalog returned {} result sets instead of {N}",
            sets.len()
        ))
    })
}

fn relations(
    [objects, columns, index_rows, fk_rows]: [Vec<Row>; 4],
) -> Result<Vec<(String, bool, Relation)>> {
    let mut columns = Columns::read(&columns)?;
    let mut indexes = indexes(&index_rows, &mut columns)?;
    let mut foreign_keys = foreign_keys(&fk_rows, &columns)?;
    objects
        .iter()
        .map(|r| {
            let object = id(r, 0)?;
            let relation = Relation {
                name: text(r, 2)?,
                columns: columns.take(object),
                indexes: indexes.remove(&object).unwrap_or_default(),
                foreign_keys: foreign_keys.remove(&object).unwrap_or_default(),
                #[allow(clippy::cast_precision_loss)]
                estimated_rows: get::<i64>(r, 4)?.map(|n| n as f64),
                detail: None,
            };
            Ok((text(r, 1)?, get::<i32>(r, 3)? == Some(1), relation))
        })
        .collect()
}

pub(crate) async fn relation(
    client: &mut Tds,
    namespace: &str,
    name: &str,
) -> Result<Option<Relation>> {
    let sql = format!(
        "SET NOCOUNT ON; DECLARE @id int = OBJECT_ID(@P1);{}",
        RELATIONS.replace(" OBJECT", " AND o.object_id = @id")
    );
    let quoted = format!(
        "[{}].[{}]",
        namespace.replace(']', "]]"),
        name.replace(']', "]]")
    );
    let found = relations(sets(result_sets(client, &sql, &[&quoted]).await?)?)?;
    Ok(found
        .into_iter()
        .find(|(ns, _, r)| ns == namespace && r.name == name)
        .map(|(_, _, r)| r))
}

pub(crate) async fn outline(client: &mut Tds) -> Result<Outline> {
    let [databases, current, sizes] = sets::<3>(result_sets(client, OUTLINE, &[]).await?)?;
    Ok(Outline {
        databases: databases
            .iter()
            .map(|r| text(r, 0))
            .collect::<Result<_>>()?,
        current_database: current
            .first()
            .map(|r| text(r, 0))
            .transpose()?
            .unwrap_or_default(),
        namespaces: sizes
            .iter()
            .map(|r| {
                Ok(NamespaceSize {
                    name: text(r, 0)?,
                    relations: get::<i32>(r, 1)?.map_or(0, |n| u64::try_from(n).unwrap_or(0)),
                })
            })
            .collect::<Result<_>>()?,
    })
}

pub(crate) async fn load(client: &mut Tds, only: Option<&[String]>) -> Result<Schema> {
    let names: Vec<&str> = only
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();
    let marks: Vec<String> = (1..=names.len()).map(|i| format!("@P{i}")).collect();
    let scope = if only.is_some() {
        format!(" AND s.name IN ({})", marks.join(", "))
    } else {
        String::new()
    };
    let object = if only.is_some() {
        format!(" AND SCHEMA_NAME(o.schema_id) IN ({})", marks.join(", "))
    } else {
        String::new()
    };
    if only.is_some_and(<[String]>::is_empty) {
        return Ok(Schema::default());
    }
    let sql = format!(
        "{}{}{}",
        HEAD.replace(" SCOPE", &scope),
        RELATIONS.replace(" OBJECT", &object),
        TAIL.replace("ROUTINES", ROUTINES).replace(" SCOPE", &scope)
    );
    let all = result_sets(client, &sql, &names).await?;
    let [
        databases,
        current,
        schemas,
        relation_rows,
        columns,
        index_rows,
        fk_rows,
        routines,
        parameters,
        sequences,
        types,
        triggers,
    ] = sets::<12>(all)?;

    let mut namespaces: BTreeMap<String, Namespace> = BTreeMap::new();
    for r in &schemas {
        let name = text(r, 0)?;
        namespaces.insert(name.clone(), Namespace::new(name));
    }
    for (ns, view, relation) in relations([relation_rows, columns, index_rows, fk_rows])? {
        if let Some(n) = namespaces.get_mut(&ns) {
            if view {
                n.views.push(relation);
            } else {
                n.tables.push(relation);
            }
        }
    }

    let mut signatures = signatures(&parameters)?;
    for r in &routines {
        if let Some(n) = namespaces.get_mut(get::<&str>(r, 1)?.unwrap_or_default()) {
            n.functions.push(Function {
                name: text(r, 2)?,
                signature: signatures.remove(&id(r, 0)?).unwrap_or_default(),
            });
        }
    }
    objects(&sequences, &mut namespaces, |n| &mut n.sequences)?;
    objects(&types, &mut namespaces, |n| &mut n.types)?;
    objects(&triggers, &mut namespaces, |n| &mut n.triggers)?;

    let mut ordered: Vec<Namespace> = namespaces.into_values().collect();
    ordered.sort_by_key(|n| n.name != "dbo");
    Ok(Schema {
        databases: databases
            .iter()
            .map(|r| text(r, 0))
            .collect::<Result<_>>()?,
        current_database: current
            .first()
            .map(|r| text(r, 0))
            .transpose()?
            .unwrap_or_default(),
        namespaces: ordered,
    })
}
