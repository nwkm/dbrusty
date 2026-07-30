pub mod csv;
pub mod json;
pub mod sql;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Csv,
    Json,
    Sql,
}

impl ImportFormat {
    pub fn from_extension(path: &str) -> Option<ImportFormat> {
        let ext = path.rsplit('.').next()?.to_lowercase();
        match ext.as_str() {
            "csv" => Some(ImportFormat::Csv),
            "json" => Some(ImportFormat::Json),
            "sql" => Some(ImportFormat::Sql),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ImportFormat::Csv => "CSV",
            ImportFormat::Json => "JSON",
            ImportFormat::Sql => "SQL",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportPreview {
    pub estimated_rows: usize,
    pub columns: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub sql_statements: Vec<String>,
}

pub fn import_data(
    format: ImportFormat,
    path: &str,
    table_name: &str,
) -> Result<ImportPreview, String> {
    match format {
        ImportFormat::Csv => csv::import_csv(path, table_name),
        ImportFormat::Json => json::import_json(path, table_name),
        ImportFormat::Sql => sql::import_sql(path),
    }
}
