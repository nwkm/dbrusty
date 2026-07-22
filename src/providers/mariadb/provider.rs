use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::{Column, Executor, Row, TypeInfo};

use crate::database::provider::{DatabaseError, DatabaseProvider, DatabaseResult};
use crate::database::result::{QueryResult, Value};
use crate::database::types::*;
use crate::models::connection::ConnectionConfig;

pub struct MariadbProvider {
    pool: Option<MySqlPool>,
}

impl MariadbProvider {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> DatabaseResult<&MySqlPool> {
        self.pool.as_ref().ok_or(DatabaseError::NotConnected)
    }
}

fn col_str(row: &MySqlRow, col: &str) -> String {
    row.try_get::<String, _>(col)
        .or_else(|_| {
            row.try_get::<Vec<u8>, _>(col)
                .map(|b| String::from_utf8_lossy(&b).to_string())
        })
        .unwrap_or_default()
}

fn col_opt_str(row: &MySqlRow, col: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(col)
        .ok()
        .flatten()
        .or_else(|| {
            row.try_get::<Option<Vec<u8>>, _>(col)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).to_string())
        })
}

fn value_from_row(row: &MySqlRow, idx: usize) -> Value {
    row.try_get::<bool, _>(idx)
        .map(Value::Bool)
        .or_else(|_| row.try_get::<i8, _>(idx).map(|v| Value::Integer(v as i64)))
        .or_else(|_| row.try_get::<i16, _>(idx).map(|v| Value::Integer(v as i64)))
        .or_else(|_| row.try_get::<i32, _>(idx).map(|v| Value::Integer(v as i64)))
        .or_else(|_| row.try_get::<i64, _>(idx).map(Value::Integer))
        .or_else(|_| row.try_get::<u64, _>(idx).map(|v| Value::Integer(v as i64)))
        .or_else(|_| row.try_get::<f32, _>(idx).map(|v| Value::Float(v as f64)))
        .or_else(|_| row.try_get::<f64, _>(idx).map(Value::Float))
        .or_else(|_| {
            row.try_get::<Decimal, _>(idx)
                .map(|d| Value::Text(d.to_string()))
        })
        .or_else(|_| {
            row.try_get::<chrono::NaiveDate, _>(idx)
                .map(|d| Value::Text(d.format("%Y-%m-%d").to_string()))
        })
        .or_else(|_| {
            row.try_get::<chrono::NaiveDateTime, _>(idx)
                .map(|dt| Value::Text(dt.format("%Y-%m-%d %H:%M:%S").to_string()))
        })
        .or_else(|_| {
            row.try_get::<chrono::NaiveTime, _>(idx)
                .map(|t| Value::Text(t.format("%H:%M:%S").to_string()))
        })
        .or_else(|_| row.try_get::<String, _>(idx).map(Value::Text))
        .or_else(|_| {
            row.try_get::<serde_json::Value, _>(idx)
                .map(|v| Value::Json(v.to_string()))
        })
        .or_else(|_| {
            row.try_get::<Vec<u8>, _>(idx)
                .map(|b| Value::Text(String::from_utf8_lossy(&b).to_string()))
        })
        .unwrap_or(Value::Null)
}

fn row_to_values(row: &MySqlRow) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| value_from_row(row, i))
        .collect()
}

fn columns_from_row(row: &MySqlRow) -> Vec<ColumnInfo> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(i, col)| ColumnInfo {
            name: col.name().to_string(),
            ordinal: i + 1,
            type_name: col.type_info().name().to_string(),
            nullable: true,
            is_pk: false,
            is_fk: false,
            default: None,
            is_auto_increment: false,
        })
        .collect()
}

fn encode_url_component(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' | '~' => result.push(c),
            _ => {
                for b in c.to_string().bytes() {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    result
}

#[async_trait]
impl DatabaseProvider for MariadbProvider {
    async fn connect(&mut self, config: &ConnectionConfig) -> DatabaseResult<()> {
        let password = match &config.password {
            Some(p) if !p.is_empty() => p.clone(),
            _ => {
                if config.username.is_empty() {
                    String::new()
                } else {
                    match crate::storage::credentials::get_password(&config.password_key()) {
                        Ok(Some(p)) => p,
                        Ok(None) => {
                            tracing::warn!("No password found in keychain for {}", config.name);
                            String::new()
                        }
                        Err(e) => {
                            tracing::warn!("Keychain error for {}: {}", config.name, e);
                            String::new()
                        }
                    }
                }
            }
        };

        let encoded_user = encode_url_component(&config.username);
        let encoded_pass = encode_url_component(&password);
        let url = format!(
            "mysql://{}:{}@{}:{}/{}",
            encoded_user, encoded_pass, config.host, config.port, config.database,
        );

        tracing::debug!(
            "Connecting to MariaDB at {}:{}/{} as {}",
            config.host,
            config.port,
            config.database,
            config.username,
        );

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&url)
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
            || trimmed.starts_with("WITH")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("SHOW");

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
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT SCHEMA_NAME AS db FROM information_schema.SCHEMATA \
             WHERE SCHEMA_NAME NOT IN ('mysql', 'performance_schema', 'sys') \
             ORDER BY SCHEMA_NAME",
        )
        .fetch_all(pool)
        .await?;

        let databases: Vec<String> = rows.iter().map(|r| col_str(&r, "db")).collect();
        Ok(databases)
    }

    async fn get_schemas(&self) -> DatabaseResult<Vec<String>> {
        self.get_databases().await
    }

    async fn get_tables(&self, schema: &str) -> DatabaseResult<Vec<TableInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT TABLE_NAME AS name, TABLE_TYPE AS table_type \
             FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = ? AND TABLE_TYPE IN ('BASE TABLE', 'VIEW') \
             ORDER BY TABLE_NAME",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let tables = rows
            .iter()
            .map(|r| {
                let name = col_str(&r, "name");
                let table_type = col_str(&r, "table_type");
                TableInfo {
                    name,
                    schema: schema.to_string(),
                    table_type,
                }
            })
            .collect();
        Ok(tables)
    }

    async fn get_columns(&self, table: &str) -> DatabaseResult<Vec<ColumnInfo>> {
        let (schema, table_name) = parse_schema_table(table);
        let pool = self.pool()?;

        let rows = sqlx::query(
            "SELECT \
             c.ORDINAL_POSITION AS ordinal, \
             c.COLUMN_NAME AS name, \
             c.COLUMN_TYPE AS type_name, \
             c.IS_NULLABLE AS is_nullable, \
             c.COLUMN_DEFAULT AS default_value, \
             CASE WHEN c.EXTRA LIKE '%auto_increment%' THEN 1 ELSE 0 END AS is_auto_inc, \
             CASE WHEN s.COLUMN_NAME IS NOT NULL AND s.INDEX_NAME = 'PRIMARY' THEN 1 ELSE 0 END AS is_pk, \
             CASE WHEN fk.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS is_fk \
             FROM information_schema.COLUMNS c \
             LEFT JOIN information_schema.STATISTICS s \
               ON s.TABLE_SCHEMA = c.TABLE_SCHEMA \
              AND s.TABLE_NAME = c.TABLE_NAME \
              AND s.COLUMN_NAME = c.COLUMN_NAME \
              AND s.INDEX_NAME = 'PRIMARY' \
             LEFT JOIN information_schema.KEY_COLUMN_USAGE fk \
               ON fk.TABLE_SCHEMA = c.TABLE_SCHEMA \
              AND fk.TABLE_NAME = c.TABLE_NAME \
              AND fk.COLUMN_NAME = c.COLUMN_NAME \
              AND fk.REFERENCED_TABLE_NAME IS NOT NULL \
             WHERE c.TABLE_SCHEMA = ? AND c.TABLE_NAME = ? \
             ORDER BY c.ORDINAL_POSITION",
        )
        .bind(&schema)
        .bind(&table_name)
        .fetch_all(pool)
        .await?;

        let columns = rows
            .iter()
            .map(|r| {
                let ordinal: u64 = r.get("ordinal");
                let name = col_str(&r, "name");
                let type_name = col_str(&r, "type_name");
                let nullable_str = col_str(&r, "is_nullable");
                let is_pk: bool = r.get("is_pk");
                let is_fk: bool = r.get("is_fk");
                let default_value = col_opt_str(&r, "default_value");
                let is_auto_inc: bool = r.get("is_auto_inc");
                ColumnInfo {
                    name,
                    ordinal: ordinal as usize,
                    type_name,
                    nullable: nullable_str == "YES",
                    is_pk,
                    is_fk,
                    default: default_value,
                    is_auto_increment: is_auto_inc,
                }
            })
            .collect();
        Ok(columns)
    }

    async fn get_indexes(&self, table: &str) -> DatabaseResult<Vec<IndexInfo>> {
        let (schema, table_name) = parse_schema_table(table);
        let pool = self.pool()?;

        let rows = sqlx::query(
            "SELECT INDEX_NAME AS index_name, COLUMN_NAME AS column_name, \
             NON_UNIQUE AS non_unique, \
             CASE WHEN INDEX_NAME = 'PRIMARY' THEN 1 ELSE 0 END AS is_primary \
             FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
             ORDER BY INDEX_NAME, SEQ_IN_INDEX",
        )
        .bind(&schema)
        .bind(&table_name)
        .fetch_all(pool)
        .await?;

        let mut index_map: std::collections::BTreeMap<String, (bool, Vec<String>, bool)> =
            std::collections::BTreeMap::new();
        for r in &rows {
            let idx_name = col_str(&r, "index_name");
            let col_name = col_str(&r, "column_name");
            let non_unique: bool = r.get("non_unique");
            let is_primary: bool = r.get("is_primary");

            let entry = index_map
                .entry(idx_name)
                .or_insert((non_unique, Vec::new(), is_primary));
            entry.1.push(col_name);
        }

        let indexes = index_map
            .into_iter()
            .map(|(name, (non_unique, cols, is_primary))| IndexInfo {
                name,
                columns: cols,
                unique: !non_unique,
                primary: is_primary,
            })
            .collect();
        Ok(indexes)
    }

    async fn get_views(&self, schema: &str) -> DatabaseResult<Vec<ViewInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT TABLE_NAME AS name, VIEW_DEFINITION AS definition \
             FROM information_schema.VIEWS \
             WHERE TABLE_SCHEMA = ? \
             ORDER BY TABLE_NAME",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let views = rows
            .iter()
            .map(|r| {
                let name = col_str(&r, "name");
                let definition = col_opt_str(&r, "definition");
                ViewInfo {
                    name,
                    definition: definition.unwrap_or_default(),
                    schema: schema.to_string(),
                }
            })
            .collect();
        Ok(views)
    }

    async fn get_sequences(&self, schema: &str) -> DatabaseResult<Vec<SequenceInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT SEQUENCE_NAME AS name, SEQUENCE_SCHEMA AS schema_name \
             FROM information_schema.SEQUENCES \
             WHERE SEQUENCE_SCHEMA = ? \
             ORDER BY SEQUENCE_NAME",
        )
        .bind(schema)
        .fetch_all(pool)
        .await;
        match rows {
            Ok(r) => {
                let sequences = r
                    .iter()
                    .map(|r| {
                let name = col_str(&r, "name");
                        SequenceInfo {
                            name,
                            schema: schema.to_string(),
                            data_type: "bigint".to_string(),
                            start_value: None,
                            increment: None,
                        }
                    })
                    .collect();
                Ok(sequences)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    async fn get_triggers(&self, schema: &str) -> DatabaseResult<Vec<TriggerInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT \
             TRIGGER_NAME AS name, \
             EVENT_OBJECT_TABLE AS table_name, \
             EVENT_MANIPULATION AS event, \
             ACTION_TIMING AS timing, \
             ACTION_ORIENTATION AS orientation, \
             ACTION_STATEMENT AS definition \
             FROM information_schema.TRIGGERS \
             WHERE TRIGGER_SCHEMA = ? \
             ORDER BY TRIGGER_NAME",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let triggers = rows
            .iter()
            .map(|r| {
                let name = col_str(&r, "name");
                let table = col_str(&r, "table_name");
                let event = col_str(&r, "event");
                let timing = col_str(&r, "timing");
                let orientation = col_str(&r, "orientation");
                let definition = col_str(&r, "definition");
                TriggerInfo {
                    name,
                    table,
                    schema: schema.to_string(),
                    event,
                    timing,
                    orientation,
                    definition,
                }
            })
            .collect();
        Ok(triggers)
    }

    async fn get_functions(&self, schema: &str) -> DatabaseResult<Vec<FunctionInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT ROUTINE_NAME AS name, DTD_IDENTIFIER AS return_type, \
             ROUTINE_DEFINITION AS definition, ROUTINE_BODY AS language \
             FROM information_schema.ROUTINES \
             WHERE ROUTINE_SCHEMA = ? AND ROUTINE_TYPE = 'FUNCTION' \
             ORDER BY ROUTINE_NAME",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let functions = rows
            .iter()
            .map(|r| {
                let name = col_str(&r, "name");
                let return_type = col_opt_str(&r, "return_type");
                let definition = col_opt_str(&r, "definition");
                let language = col_str(&r, "language");
                FunctionInfo {
                    name,
                    schema: schema.to_string(),
                    return_type,
                    language,
                    args: Vec::new(),
                    definition,
                    is_strict: false,
                    is_security_definer: false,
                }
            })
            .collect();
        Ok(functions)
    }

    async fn get_procedures(&self, schema: &str) -> DatabaseResult<Vec<ProcedureInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT ROUTINE_NAME AS name, ROUTINE_DEFINITION AS definition \
             FROM information_schema.ROUTINES \
             WHERE ROUTINE_SCHEMA = ? AND ROUTINE_TYPE = 'PROCEDURE' \
             ORDER BY ROUTINE_NAME",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let procedures = rows
            .iter()
            .map(|r| {
                        let name = col_str(&r, "name");
                ProcedureInfo {
                    name,
                    params: Vec::new(),
                }
            })
            .collect();
        Ok(procedures)
    }

    async fn get_extensions(&self) -> DatabaseResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn get_custom_types(&self, schema: &str) -> DatabaseResult<Vec<TypeSchemaInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT DISTINCT c.COLUMN_TYPE AS type_name \
             FROM information_schema.COLUMNS c \
             WHERE c.TABLE_SCHEMA = ? \
             AND (c.COLUMN_TYPE LIKE 'enum%' OR c.COLUMN_TYPE LIKE 'set%') \
             ORDER BY c.COLUMN_TYPE",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let types = rows
            .iter()
            .map(|r| {
                let type_name = col_str(&r, "type_name");
                let kind = if type_name.starts_with("enum") {
                    "enum"
                } else {
                    "set"
                };
                TypeSchemaInfo {
                    name: type_name,
                    schema: schema.to_string(),
                    kind: kind.to_string(),
                }
            })
            .collect();
        Ok(types)
    }

    async fn begin_transaction(&self) -> DatabaseResult<()> {
        let pool = self.pool()?;
        let mut conn = pool.acquire().await?;
        conn.execute("BEGIN").await?;
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
        if let Ok(pool) = self.pool() {
            let row = sqlx::query("SELECT CONNECTION_ID() AS id")
                .fetch_one(pool)
                .await
                .ok();
            if let Some(r) = row {
                if let Ok(conn_id) = r.try_get::<u64, _>("id") {
                    let kill_sql = format!("KILL QUERY {}", conn_id);
                    let _ = pool.execute(kill_sql.as_str()).await;
                }
            }
        }
        Ok(())
    }

    async fn server_info(&self) -> DatabaseResult<ServerInfo> {
        let pool = self.pool()?;
        let version_row = sqlx::query("SELECT VERSION() AS version")
            .fetch_one(pool)
            .await?;
        let version = col_str(&version_row, "version");

        Ok(ServerInfo {
            version,
            vendor: "MariaDB".to_string(),
            connection_uptime: None,
        })
    }
}

fn parse_schema_table(identifier: &str) -> (String, String) {
    if let Some(dot_pos) = identifier.rfind('.') {
        let schema = &identifier[..dot_pos];
        let table = &identifier[dot_pos + 1..];
        (schema.to_string(), table.to_string())
    } else {
        ("public".to_string(), identifier.to_string())
    }
}
