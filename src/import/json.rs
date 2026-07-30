use std::fs;

use super::ImportPreview;

pub fn import_json(path: &str, table_name: &str) -> Result<ImportPreview, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;

    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("JSON parse error: {}", e))?;

    let records = match &value {
        serde_json::Value::Array(arr) => arr,
        _ => return Err("JSON file must contain an array of objects".to_string()),
    };

    if records.is_empty() {
        return Ok(ImportPreview {
            estimated_rows: 0,
            columns: Vec::new(),
            sample_rows: Vec::new(),
            sql_statements: Vec::new(),
        });
    }

    let mut col_set: Vec<String> = Vec::new();
    for rec in records {
        if let serde_json::Value::Object(map) = rec {
            for key in map.keys() {
                if !col_set.contains(key) {
                    col_set.push(key.clone());
                }
            }
        }
    }
    let headers = col_set;
    let col_list = headers.join(", ");

    let mut sample_rows: Vec<Vec<String>> = Vec::new();
    let mut total_rows = 0;
    let mut sql_statements = Vec::new();

    for rec in records {
        if let serde_json::Value::Object(map) = rec {
            let row: Vec<String> = headers
                .iter()
                .map(|col| {
                    map.get(col)
                        .map(|v| match v {
                            serde_json::Value::Null => "NULL".to_string(),
                            serde_json::Value::String(s) => {
                                format!("'{}'", crate::export::escape_sql_string(s))
                            }
                            _ => {
                                format!("'{}'", crate::export::escape_sql_string(&v.to_string()))
                            }
                        })
                        .unwrap_or_else(|| "NULL".to_string())
                })
                .collect();

            let val_list = row.join(", ");
            sql_statements.push(format!(
                "INSERT INTO {table_name} ({col_list}) VALUES ({val_list});"
            ));

            if total_rows < 5 {
                let display_row: Vec<String> = headers
                    .iter()
                    .map(|col| map.get(col).map(|v| v.to_string()).unwrap_or_default())
                    .collect();
                sample_rows.push(display_row);
            }
            total_rows += 1;
        }
    }

    Ok(ImportPreview {
        estimated_rows: total_rows,
        columns: headers,
        sample_rows,
        sql_statements,
    })
}
