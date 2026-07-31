use std::collections::HashMap;

use crate::database::types::{
    ColumnInfo, FunctionInfo, IndexInfo, ProcedureInfo, SequenceInfo, TableInfo, TriggerInfo,
    ViewInfo,
};

#[derive(Debug, Clone, Default)]
pub struct MetadataCache {
    pub(crate) tables: HashMap<String, Vec<TableInfo>>,
    pub(crate) columns: HashMap<String, Vec<ColumnInfo>>,
    pub(crate) views: HashMap<String, Vec<ViewInfo>>,
    pub(crate) indexes: HashMap<String, Vec<IndexInfo>>,
    pub(crate) sequences: HashMap<String, Vec<SequenceInfo>>,
    pub(crate) triggers: HashMap<String, Vec<TriggerInfo>>,
    pub(crate) functions: HashMap<String, Vec<FunctionInfo>>,
    pub(crate) procedures: HashMap<String, Vec<ProcedureInfo>>,
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

    pub fn set_sequences(&mut self, schema: String, sequences: Vec<SequenceInfo>) {
        self.sequences.insert(schema, sequences);
    }

    pub fn set_triggers(&mut self, schema: String, triggers: Vec<TriggerInfo>) {
        self.triggers.insert(schema, triggers);
    }

    pub fn set_functions(&mut self, schema: String, functions: Vec<FunctionInfo>) {
        self.functions.insert(schema, functions);
    }

    pub fn set_procedures(&mut self, schema: String, procedures: Vec<ProcedureInfo>) {
        self.procedures.insert(schema, procedures);
    }
}
