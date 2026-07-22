use async_trait::async_trait;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Column, Executor, Row};

use crate::database::provider::{DatabaseError, DatabaseProvider, DatabaseResult};
use crate::database::result::{QueryResult, Value};
use crate::database::types::*;
use crate::models::connection::ConnectionConfig;

pub struct SqliteProvider {
    pool: Option<sqlx::SqlitePool>,
}

impl SqliteProvider {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> DatabaseResult<&sqlx::SqlitePool> {
        self.pool.as_ref().ok_or(DatabaseError::NotConnected)
    }
}

fn value_from_row(row: &sqlx::sqlite::SqliteRow, idx: usize) -> Value {
    row.try_get::<i64, _>(idx)
        .map(Value::Integer)
        .or_else(|_| row.try_get::<f64, _>(idx).map(Value::Float))
        .or_else(|_| row.try_get::<String, _>(idx).map(Value::Text))
        .or_else(|_| row.try_get::<bool, _>(idx).map(Value::Bool))
        .unwrap_or(Value::Null)
}

fn parse_schema_table(identifier: &str) -> (String, String) {
    if let Some(dot_pos) = identifier.rfind('.') {
        let schema = &identifier[..dot_pos];
        let table = &identifier[dot_pos + 1..];
        (schema.to_string(), table.to_string())
    } else {
        ("main".to_string(), identifier.to_string())
    }
}

fn row_to_values(row: &sqlx::sqlite::SqliteRow) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| value_from_row(row, i))
        .collect()
}

fn columns_from_row(row: &sqlx::sqlite::SqliteRow) -> Vec<ColumnInfo> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(i, col)| ColumnInfo {
            name: col.name().to_string(),
            ordinal: i,
            type_name: format!("{:?}", col.type_info()),
            nullable: true,
            is_pk: false,
            is_fk: false,
            default: None,
            is_auto_increment: false,
        })
        .collect()
}

#[async_trait]
impl DatabaseProvider for SqliteProvider {
    async fn connect(&mut self, config: &ConnectionConfig) -> DatabaseResult<()> {
        let conn_str = config
            .connection_string(None)
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&conn_str)
            .await
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;

        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> DatabaseResult<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        Ok(())
    }

    async fn ping(&self) -> DatabaseResult<bool> {
        let pool = self.pool()?;
        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map(|_| true)
            .map_err(DatabaseError::from)
    }

    async fn execute_query(&self, sql: &str) -> DatabaseResult<QueryResult> {
        let pool = self.pool()?;
        let start = std::time::Instant::now();

        let trimmed = sql.trim().to_uppercase();
        let is_query = trimmed.starts_with("SELECT")
            || trimmed.starts_with("PRAGMA")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("WITH");

        if is_query {
            let mut conn = pool.acquire().await?;

            let rows = conn.fetch_all(sql).await?;

            if rows.is_empty() {
                return Ok(QueryResult::new_select(
                    Vec::new(),
                    Vec::new(),
                    start.elapsed(),
                ));
            }

            let columns = columns_from_row(&rows[0]);
            let values: Vec<Vec<Value>> = rows.iter().map(row_to_values).collect();

            Ok(QueryResult::new_select(columns, values, start.elapsed()))
        } else {
            let mut conn = pool.acquire().await?;

            let result = conn.execute(sql).await?;

            Ok(QueryResult::new_execute(
                result.rows_affected(),
                start.elapsed(),
            ))
        }
    }

    async fn get_databases(&self) -> DatabaseResult<Vec<String>> {
        Ok(vec!["main".to_string()])
    }

    async fn get_schemas(&self) -> DatabaseResult<Vec<String>> {
        Ok(vec!["main".to_string()])
    }

    async fn get_tables(&self, _schema: &str) -> DatabaseResult<Vec<TableInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(pool)
        .await?;

        let tables = rows
            .iter()
            .map(|row| {
                let name: String = row.get("name");
                TableInfo {
                    name,
                    schema: "main".to_string(),
                    table_type: "TABLE".to_string(),
                }
            })
            .collect();
        Ok(tables)
    }

    async fn get_columns(&self, table: &str) -> DatabaseResult<Vec<ColumnInfo>> {
        let pool = self.pool()?;
        let (_, table_name) = parse_schema_table(table);
        let sql = format!("PRAGMA table_info(\"{}\")", table_name);
        let rows = sqlx::query(&sql).fetch_all(pool).await?;

        let columns = rows
            .iter()
            .map(|row| {
                let cid: i32 = row.get("cid");
                let name: String = row.get("name");
                let type_name: String = row.get("type");
                let notnull: bool = row.get("notnull");
                let pk: i32 = row.get("pk");
                let dflt_value: Option<String> = row.try_get("dflt_value").ok().flatten();
                ColumnInfo {
                    name,
                    ordinal: cid as usize,
                    type_name,
                    nullable: !notnull,
                    is_pk: pk != 0,
                    is_fk: false,
                    default: dflt_value,
                    is_auto_increment: false,
                }
            })
            .collect();
        Ok(columns)
    }

    async fn get_indexes(&self, table: &str) -> DatabaseResult<Vec<IndexInfo>> {
        let pool = self.pool()?;
        let (_, table_name) = parse_schema_table(table);
        let idx_sql = format!("PRAGMA index_list(\"{}\")", table_name);
        let idx_rows = sqlx::query(&idx_sql).fetch_all(pool).await?;

        let mut indexes = Vec::new();
        for row in &idx_rows {
            let name: String = row.get("name");
            let unique: bool = row.get("unique");
            let info_sql = format!("PRAGMA index_info(\"{}\")", name);
            let info_rows = sqlx::query(&info_sql).fetch_all(pool).await?;

            let columns: Vec<String> = info_rows.iter().map(|r| r.get("name")).collect();
            indexes.push(IndexInfo {
                name,
                columns,
                unique,
                primary: false,
            });
        }
        Ok(indexes)
    }

    async fn get_views(&self, _schema: &str) -> DatabaseResult<Vec<ViewInfo>> {
        let pool = self.pool()?;
        let rows =
            sqlx::query("SELECT name, sql FROM sqlite_master WHERE type='view' ORDER BY name")
                .fetch_all(pool)
                .await?;

        let views = rows
            .iter()
            .map(|row| {
                let name: String = row.get("name");
                let definition: Option<String> = row.get("sql");
                ViewInfo {
                    name,
                    definition: definition.unwrap_or_default(),
                    schema: "main".to_string(),
                }
            })
            .collect();
        Ok(views)
    }

    async fn get_procedures(&self, _schema: &str) -> DatabaseResult<Vec<ProcedureInfo>> {
        Ok(Vec::new())
    }

    async fn begin_transaction(&self) -> DatabaseResult<()> {
        let pool = self.pool()?;
        let mut conn = pool.acquire().await?;
        conn.execute("BEGIN TRANSACTION").await?;
        Ok(())
    }

    async fn commit(&self) -> DatabaseResult<()> {
        let pool = self.pool()?;
        let mut conn = pool.acquire().await?;
        conn.execute("COMMIT").await?;
        Ok(())
    }

    async fn rollback(&self) -> DatabaseResult<()> {
        let pool = self.pool()?;
        let mut conn = pool.acquire().await?;
        conn.execute("ROLLBACK").await?;
        Ok(())
    }

    async fn cancel_query(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn server_info(&self) -> DatabaseResult<ServerInfo> {
        let pool = self.pool()?;
        let row = sqlx::query("SELECT sqlite_version() as version")
            .fetch_one(pool)
            .await?;
        let version: String = row.get("version");
        Ok(ServerInfo {
            version,
            vendor: "SQLite".to_string(),
            connection_uptime: None,
        })
    }
}
