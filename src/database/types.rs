#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub table_type: String,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub ordinal: usize,
    pub type_name: String,
    pub nullable: bool,
    pub is_pk: bool,
    pub is_fk: bool,
    pub default: Option<String>,
    pub is_auto_increment: bool,
}

#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Debug, Clone)]
pub struct ViewInfo {
    pub name: String,
    pub definition: String,
    pub schema: String,
}

#[derive(Debug, Clone)]
pub struct ProcedureInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub type_name: String,
    pub mode: String,
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub version: String,
    pub vendor: String,
    pub connection_uptime: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SequenceInfo {
    pub name: String,
    pub schema: String,
    pub data_type: String,
    pub start_value: Option<i64>,
    pub increment: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TriggerInfo {
    pub name: String,
    pub table: String,
    pub schema: String,
    pub event: String,
    pub timing: String,
    pub orientation: String,
    pub definition: String,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub schema: String,
    pub return_type: Option<String>,
    pub language: String,
    pub args: Vec<ParamInfo>,
    pub definition: Option<String>,
    pub is_strict: bool,
    pub is_security_definer: bool,
}

#[derive(Debug, Clone)]
pub struct TypeSchemaInfo {
    pub name: String,
    pub schema: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub update_rule: Option<String>,
    pub delete_rule: Option<String>,
}
