use std::fs::File;

use super::ImportPreview;

pub fn import_csv(path: &str, table_name: &str) -> Result<ImportPreview, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {}", path, e))?;
    let mut rdr = csv::Reader::from_reader(file);

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("CSV header error: {}", e))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        return Err("CSV file has no columns".to_string());
    }

    let mut sample_rows: Vec<Vec<String>> = Vec::new();
    let mut total_rows = 0;

    let col_list = headers.join(", ");
    let mut sql_statements = Vec::new();

    for result in rdr.records() {
        let record = result.map_err(|e| format!("CSV read error: {}", e))?;
        let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();

        let vals: Vec<String> = row
            .iter()
            .map(|v| format!("'{}'", crate::export::escape_sql_string(v)))
            .collect();
        let val_list = vals.join(", ");
        sql_statements.push(format!(
            "INSERT INTO {table_name} ({col_list}) VALUES ({val_list});"
        ));

        if total_rows < 5 {
            sample_rows.push(row);
        }
        total_rows += 1;
    }

    if total_rows == 0 {
        return Ok(ImportPreview {
            estimated_rows: 0,
            columns: headers,
            sample_rows: Vec::new(),
            sql_statements: Vec::new(),
        });
    }

    Ok(ImportPreview {
        estimated_rows: total_rows,
        columns: headers,
        sample_rows,
        sql_statements,
    })
}
