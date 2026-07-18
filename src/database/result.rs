use std::time::Duration;

use crate::database::types::ColumnInfo;

#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
    Null,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Text(_) => "string",
            Value::Blob(_) => "blob",
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Integer(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::Text(v) => write!(f, "{}", v),
            Value::Blob(_) => write!(f, "[BLOB]"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Value>>,
    pub rows_affected: u64,
    pub duration: Duration,
    pub messages: Vec<String>,
    pub is_select: bool,
}

impl QueryResult {
    pub fn new_select(columns: Vec<ColumnInfo>, rows: Vec<Vec<Value>>, duration: Duration) -> Self {
        Self {
            columns,
            rows: rows.into_iter().map(|r| r.into_iter().collect()).collect(),
            rows_affected: 0,
            duration,
            messages: Vec::new(),
            is_select: true,
        }
    }

    pub fn new_execute(rows_affected: u64, duration: Duration) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected,
            duration,
            messages: Vec::new(),
            is_select: false,
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn col_count(&self) -> usize {
        self.columns.len()
    }
}
