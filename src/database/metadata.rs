use std::collections::HashMap;

use crate::database::types::{ColumnInfo, IndexInfo, TableInfo, ViewInfo};

#[derive(Debug, Clone, Default)]
pub struct MetadataCache {
    pub tables: HashMap<String, Vec<TableInfo>>,
    pub columns: HashMap<String, Vec<ColumnInfo>>,
    pub views: HashMap<String, Vec<ViewInfo>>,
    pub indexes: HashMap<String, Vec<IndexInfo>>,
}

impl MetadataCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_columns(&self, table: &str) -> Option<&[ColumnInfo]> {
        self.columns.get(table).map(|v| v.as_slice())
    }

    pub fn set_columns(&mut self, table: String, columns: Vec<ColumnInfo>) {
        self.columns.insert(table, columns);
    }

    pub fn set_tables(&mut self, schema: String, tables: Vec<TableInfo>) {
        self.tables.insert(schema, tables);
    }

    pub fn set_views(&mut self, schema: String, views: Vec<ViewInfo>) {
        self.views.insert(schema, views);
    }

    pub fn set_indexes(&mut self, table: String, indexes: Vec<IndexInfo>) {
        self.indexes.insert(table, indexes);
    }
}
