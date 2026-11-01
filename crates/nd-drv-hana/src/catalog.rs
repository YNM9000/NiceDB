use nd_driver::CatalogRows;

fn user(column: &str) -> String {
    format!(
        "{column} IN (SELECT SCHEMA_NAME FROM SYS.SCHEMAS WHERE SCHEMA_OWNER <> 'SYS' \
AND SCHEMA_OWNER NOT LIKE '\\_SYS%' ESCAPE '\\' AND SCHEMA_NAME NOT LIKE '\\_SYS%' ESCAPE '\\' \
AND SCHEMA_NAME NOT LIKE '\\_SAP%' ESCAPE '\\')"
    )
}

pub fn relations() -> String {
    format!(
        "SELECT t.SCHEMA_NAME, t.TABLE_NAME, 'table', m.RECORD_COUNT FROM SYS.TABLES t \
LEFT JOIN SYS.M_TABLES m ON m.SCHEMA_NAME = t.SCHEMA_NAME AND m.TABLE_NAME = t.TABLE_NAME \
WHERE t.IS_SYSTEM_TABLE = 'FALSE' AND {} \
UNION ALL SELECT v.SCHEMA_NAME, v.VIEW_NAME, 'view', NULL FROM SYS.VIEWS v WHERE {} ORDER BY 1, 2",
        user("t.SCHEMA_NAME"),
        user("v.SCHEMA_NAME")
    )
}

pub fn columns_sql() -> String {
    format!(
        "SELECT SCHEMA_NAME, TABLE_NAME, COLUMN_NAME, DATA_TYPE_NAME, LENGTH, SCALE, IS_NULLABLE, POSITION \
FROM SYS.TABLE_COLUMNS WHERE {} \
UNION ALL SELECT SCHEMA_NAME, VIEW_NAME, COLUMN_NAME, DATA_TYPE_NAME, LENGTH, SCALE, IS_NULLABLE, POSITION \
FROM SYS.VIEW_COLUMNS WHERE {} ORDER BY 1, 2, 8",
        user("SCHEMA_NAME"),
        user("SCHEMA_NAME")
    )
}

pub fn indexes() -> String {
    format!(
        "SELECT SCHEMA_NAME, TABLE_NAME, INDEX_NAME, COLUMN_NAME, \
CASE WHEN CONSTRAINT LIKE '%UNIQUE%' OR CONSTRAINT = 'PRIMARY KEY' THEN 'Y' ELSE 'N' END, \
CASE WHEN CONSTRAINT = 'PRIMARY KEY' THEN 'Y' ELSE 'N' END \
FROM SYS.INDEX_COLUMNS WHERE {} ORDER BY 1, 2, 3, POSITION",
        user("SCHEMA_NAME")
    )
}

pub fn foreign_keys() -> String {
    format!(
        "SELECT SCHEMA_NAME, TABLE_NAME, CONSTRAINT_NAME, REFERENCED_SCHEMA_NAME, REFERENCED_TABLE_NAME, \
COLUMN_NAME, REFERENCED_COLUMN_NAME, DELETE_RULE, UPDATE_RULE \
FROM SYS.REFERENTIAL_CONSTRAINTS WHERE {} ORDER BY 1, 2, 3, POSITION",
        user("SCHEMA_NAME")
    )
}

pub fn routines() -> String {
    format!(
        "SELECT SCHEMA_NAME, PROCEDURE_NAME, PROCEDURE_NAME FROM SYS.PROCEDURES WHERE {} \
UNION ALL SELECT SCHEMA_NAME, FUNCTION_NAME, FUNCTION_NAME FROM SYS.FUNCTIONS WHERE {} ORDER BY 1, 2",
        user("SCHEMA_NAME"),
        user("SCHEMA_NAME")
    )
}

pub fn sequences() -> String {
    format!(
        "SELECT SCHEMA_NAME, SEQUENCE_NAME FROM SYS.SEQUENCES WHERE {} ORDER BY 1, 2",
        user("SCHEMA_NAME")
    )
}

pub fn triggers() -> String {
    format!(
        "SELECT SCHEMA_NAME, TRIGGER_NAME, SUBJECT_TABLE_NAME FROM SYS.TRIGGERS WHERE {} \
AND TRIGGER_NAME NOT LIKE '\\_SYS\\_%' ESCAPE '\\' ORDER BY 1, 2",
        user("SCHEMA_NAME")
    )
}

pub const CURRENT_SCHEMA: &str = "SELECT CURRENT_SCHEMA FROM DUMMY";
pub const VERSION: &str = "SELECT VERSION FROM SYS.M_DATABASE";
pub const DATABASE: &str = "SELECT DATABASE_NAME FROM SYS.M_DATABASE";

pub const OBJECT_DEFINITION: &str = "CALL SYS.GET_OBJECT_DEFINITION(?, ?)";

pub const VIEW_TEXT: &str =
    "SELECT DEFINITION FROM SYS.VIEWS WHERE SCHEMA_NAME = ? AND VIEW_NAME = ?";
pub fn create_view(schema: &str, name: &str, body: &str) -> String {
    let q = |n: &str| nd_sql::ident(nd_sql::Dialect::Hana, n);
    format!("CREATE VIEW {}.{} AS {}", q(schema), q(name), body.trim())
}

pub const ROUTINE_TEXT: &str = "SELECT DEFINITION FROM SYS.PROCEDURES WHERE SCHEMA_NAME = ? AND PROCEDURE_NAME = ? \
UNION ALL SELECT DEFINITION FROM SYS.FUNCTIONS WHERE SCHEMA_NAME = ? AND FUNCTION_NAME = ?";
pub const TRIGGER_TEXT: &str =
    "SELECT DEFINITION FROM SYS.TRIGGERS WHERE SCHEMA_NAME = ? AND TRIGGER_NAME = ?";

pub const DEPENDENCIES: &str = "SELECT 'uses', 'definition', \
CASE BASE_OBJECT_TYPE WHEN 'VIEW' THEN 'view' WHEN 'PROCEDURE' THEN 'routine' WHEN 'FUNCTION' THEN 'routine' \
  WHEN 'SEQUENCE' THEN 'sequence' WHEN 'TABLE' THEN 'table' END, BASE_SCHEMA_NAME, BASE_OBJECT_NAME, NULL \
FROM SYS.OBJECT_DEPENDENCIES WHERE DEPENDENT_SCHEMA_NAME = ? AND DEPENDENT_OBJECT_NAME = ? AND DEPENDENCY_TYPE = 1 \
UNION ALL SELECT 'used_by', CASE WHEN DEPENDENT_OBJECT_TYPE = 'TRIGGER' THEN 'trigger' ELSE 'definition' END, \
CASE DEPENDENT_OBJECT_TYPE WHEN 'VIEW' THEN 'view' WHEN 'PROCEDURE' THEN 'routine' WHEN 'FUNCTION' THEN 'routine' \
  WHEN 'TRIGGER' THEN 'trigger' WHEN 'TABLE' THEN 'table' END, DEPENDENT_SCHEMA_NAME, DEPENDENT_OBJECT_NAME, NULL \
FROM SYS.OBJECT_DEPENDENCIES WHERE BASE_SCHEMA_NAME = ? AND BASE_OBJECT_NAME = ? AND DEPENDENCY_TYPE = 1 \
UNION ALL SELECT DISTINCT 'uses', 'foreign_key', 'table', REFERENCED_SCHEMA_NAME, REFERENCED_TABLE_NAME, CONSTRAINT_NAME \
FROM SYS.REFERENTIAL_CONSTRAINTS WHERE SCHEMA_NAME = ? AND TABLE_NAME = ? \
UNION ALL SELECT DISTINCT 'used_by', 'foreign_key', 'table', SCHEMA_NAME, TABLE_NAME, CONSTRAINT_NAME \
FROM SYS.REFERENTIAL_CONSTRAINTS WHERE REFERENCED_SCHEMA_NAME = ? AND REFERENCED_TABLE_NAME = ?";

pub fn type_name(typename: &str, length: &str, scale: &str) -> String {
    let t = typename.trim().to_ascii_uppercase();
    let (len, sc) = (length.trim(), scale.trim());
    match t.as_str() {
        "VARCHAR" | "NVARCHAR" | "CHAR" | "NCHAR" | "VARBINARY" | "BINARY" | "ALPHANUM"
        | "SHORTTEXT" => format!("{t}({len})"),
        "DECIMAL" if sc.is_empty() || len.is_empty() => "DECIMAL".into(),
        "DECIMAL" => format!("DECIMAL({len},{sc})"),
        _ => t,
    }
}

pub fn catalog_sql() -> [String; 7] {
    [
        relations(),
        columns_sql(),
        indexes(),
        foreign_keys(),
        routines(),
        sequences(),
        triggers(),
    ]
}

pub fn catalog(mut answers: Vec<Vec<Vec<Option<String>>>>) -> CatalogRows {
    let mut take = || {
        if answers.is_empty() {
            Vec::new()
        } else {
            answers.remove(0)
        }
    };
    let relations = take();
    let columns = take()
        .into_iter()
        .map(|r| {
            let cell = |i: usize| r.get(i).cloned().flatten().unwrap_or_default();
            let nullable = if cell(6).eq_ignore_ascii_case("FALSE") {
                "N"
            } else {
                "Y"
            };
            vec![
                Some(cell(0)),
                Some(cell(1)),
                Some(cell(2)),
                Some(type_name(&cell(3), &cell(4), &cell(5))),
                Some(nullable.to_owned()),
            ]
        })
        .collect();
    CatalogRows {
        relations,
        columns,
        indexes: take(),
        foreign_keys: take(),
        routines: take(),
        sequences: take(),
        triggers: take(),
    }
}
