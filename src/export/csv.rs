use std::fs::File;

pub fn export_csv(columns: &[String], rows: &[Vec<String>], filename: &str) -> Result<(), String> {
    let file = File::create(filename).map_err(|e| format!("Cannot create {}: {}", filename, e))?;
    let mut wtr = csv::Writer::from_writer(file);

    wtr.write_record(columns)
        .map_err(|e| format!("CSV write header error: {}", e))?;

    for row in rows {
        wtr.write_record(row)
            .map_err(|e| format!("CSV write row error: {}", e))?;
    }

    wtr.flush().map_err(|e| format!("CSV flush error: {}", e))?;
    Ok(())
}
