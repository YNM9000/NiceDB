use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub tables: Vec<TableFacts>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableFacts {
    pub namespace: String,
    pub name: String,
    pub rows: Option<f64>,
    pub columns: Vec<ColumnFacts>,
    pub indexes: Vec<Vec<String>>,
    pub foreign_keys: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColumnFacts {
    pub name: String,
    pub type_name: String,
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl Catalog {
    pub fn table(&self, name: &str) -> Option<&TableFacts> {
        let name = name.rsplit('.').next().unwrap_or(name);
        self.tables.iter().find(|t| same(&t.name, name))
    }
}

impl TableFacts {
    pub fn leads_index(&self, column: &str) -> bool {
        self.indexes
            .iter()
            .any(|cols| cols.first().is_some_and(|c| same(c, column)))
    }

    pub fn in_foreign_key(&self, column: &str) -> bool {
        self.foreign_keys
            .iter()
            .any(|cols| cols.first().is_some_and(|c| same(c, column)))
    }

    pub fn column(&self, name: &str) -> Option<&ColumnFacts> {
        self.columns.iter().find(|c| same(&c.name, name))
    }

    pub fn is_textual(&self, column: &str) -> bool {
        self.column(column).is_some_and(|c| {
            let t = c.type_name.to_ascii_lowercase();
            ["char", "text", "string", "clob", "enum"]
                .iter()
                .any(|k| t.contains(k))
        })
    }
}

impl From<&nd_driver::Schema> for Catalog {
    fn from(schema: &nd_driver::Schema) -> Self {
        let tables = schema
            .namespaces
            .iter()
            .flat_map(|ns| {
                ns.tables.iter().map(|t| TableFacts {
                    namespace: ns.name.clone(),
                    name: t.name.clone(),
                    rows: t.estimated_rows,
                    columns: t
                        .columns
                        .iter()
                        .map(|c| ColumnFacts {
                            name: c.name.clone(),
                            type_name: c.type_name.clone(),
                        })
                        .collect(),
                    indexes: t
                        .indexes
                        .iter()
                        .map(|i| i.columns.clone())
                        .chain(
                            t.columns
                                .iter()
                                .filter(|c| c.primary_key)
                                .take(1)
                                .map(|c| vec![c.name.clone()]),
                        )
                        .collect(),
                    foreign_keys: t.foreign_keys.iter().map(|f| f.columns.clone()).collect(),
                })
            })
            .collect();
        Self { tables }
    }
}
