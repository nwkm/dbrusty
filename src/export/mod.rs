pub mod csv;
pub mod json;
pub mod sql;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
    Sql,
}

impl ExportFormat {
    pub fn label(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Json => "JSON",
            ExportFormat::Sql => "SQL",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::Sql => "sql",
        }
    }

    pub fn all() -> [ExportFormat; 3] {
        [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Sql]
    }
}

pub fn export_data(
    format: ExportFormat,
    columns: &[String],
    rows: &[Vec<String>],
    filename: &str,
) -> Result<(), String> {
    match format {
        ExportFormat::Csv => csv::export_csv(columns, rows, filename),
        ExportFormat::Json => json::export_json(columns, rows, filename),
        ExportFormat::Sql => sql::export_sql(columns, rows, filename),
    }
}

pub fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}
