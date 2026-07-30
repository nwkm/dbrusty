use std::fs::File;
use std::io::Write;

use serde_json::json;

pub fn export_json(columns: &[String], rows: &[Vec<String>], filename: &str) -> Result<(), String> {
    let file = File::create(filename).map_err(|e| format!("Cannot create {}: {}", filename, e))?;
    let mut writer = std::io::BufWriter::new(file);

    let records: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut map = serde_json::Map::new();
            for (i, col) in columns.iter().enumerate() {
                let val = row.get(i).map(|s| s.as_str()).unwrap_or("");
                map.insert(col.clone(), json!(val));
            }
            serde_json::Value::Object(map)
        })
        .collect();

    let bytes =
        serde_json::to_vec_pretty(&records).map_err(|e| format!("JSON serialize error: {}", e))?;
    writer
        .write_all(&bytes)
        .map_err(|e| format!("JSON write error: {}", e))?;

    Ok(())
}
