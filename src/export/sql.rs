use std::fs::File;
use std::io::Write;

use super::escape_sql_string;

pub fn export_sql(columns: &[String], rows: &[Vec<String>], filename: &str) -> Result<(), String> {
    let file = File::create(filename).map_err(|e| format!("Cannot create {}: {}", filename, e))?;
    let mut writer = std::io::BufWriter::new(file);

    let col_list = columns.join(", ");

    for row in rows {
        let vals: Vec<String> = row
            .iter()
            .map(|v| {
                if v == "NULL" {
                    "NULL".to_string()
                } else {
                    format!("'{}'", escape_sql_string(v))
                }
            })
            .collect();
        let val_list = vals.join(", ");
        writeln!(
            writer,
            "INSERT INTO export ({col_list}) VALUES ({val_list});"
        )
        .map_err(|e| format!("SQL write error: {}", e))?;
    }

    Ok(())
}
