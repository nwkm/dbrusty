use std::time::Duration;

use crate::database::types::ColumnInfo;

#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Json(String),
    Blob(Vec<u8>),
    Null,
}

fn compact_json(s: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(serde_json::Value::Object(map)) => {
            if map.len() <= 3 {
                let pairs: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, compact_json_val(v)))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            } else {
                let preview: Vec<String> = map.keys().take(2).cloned().collect();
                let remaining = map.len() - preview.len();
                format!("{{{}, +{}}}", preview.join(", "), remaining)
            }
        }
        Ok(serde_json::Value::Array(arr)) => {
            if arr.len() <= 3 {
                let items: Vec<String> = arr.iter().map(compact_json_val).collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("[{} items]", arr.len())
            }
        }
        _ => s.to_string(),
    }
}

fn compact_json_val(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            let truncated: String = s.chars().take(40).collect();
            if truncated.len() < s.len() {
                format!("\"{}…\"", truncated)
            } else {
                format!("\"{}\"", truncated)
            }
        }
        serde_json::Value::Object(_) => "{…}".to_string(),
        serde_json::Value::Array(_) => "[…]".to_string(),
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Text(_) => "string",
            Value::Json(s) => {
                if s.starts_with('{') {
                    "object"
                } else if s.starts_with('[') {
                    "array"
                } else {
                    "json"
                }
            }
            Value::Blob(_) => "blob",
        }
    }

    pub fn json_str(&self) -> Option<&str> {
        match self {
            Value::Json(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::Integer(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Text(v) => write!(f, "{v}"),
            Value::Json(v) => write!(f, "{}", compact_json(v)),
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
