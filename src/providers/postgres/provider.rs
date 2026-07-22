use async_trait::async_trait;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Executor, PgPool, Row};

use crate::database::provider::{DatabaseError, DatabaseProvider, DatabaseResult};
use crate::database::result::{QueryResult, Value};
use crate::database::types::*;
use crate::models::connection::ConnectionConfig;

pub struct PostgresProvider {
    pool: Option<PgPool>,
}

impl PostgresProvider {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> DatabaseResult<&PgPool> {
        self.pool.as_ref().ok_or(DatabaseError::NotConnected)
    }
}

fn value_from_row(row: &PgRow, idx: usize) -> Value {
    row.try_get::<bool, _>(idx)
        .map(Value::Bool)
        .or_else(|_| row.try_get::<i16, _>(idx).map(|v| Value::Integer(v as i64)))
        .or_else(|_| row.try_get::<i32, _>(idx).map(|v| Value::Integer(v as i64)))
        .or_else(|_| row.try_get::<i64, _>(idx).map(Value::Integer))
        .or_else(|_| row.try_get::<f32, _>(idx).map(|v| Value::Float(v as f64)))
        .or_else(|_| row.try_get::<f64, _>(idx).map(Value::Float))
        .or_else(|_| {
            row.try_get::<chrono::NaiveDate, _>(idx)
                .map(|d| Value::Text(d.format("%Y-%m-%d").to_string()))
        })
        .or_else(|_| {
            row.try_get::<chrono::NaiveDateTime, _>(idx)
                .map(|dt| Value::Text(dt.format("%Y-%m-%d %H:%M:%S").to_string()))
        })
        .or_else(|_| {
            row.try_get::<chrono::DateTime<chrono::Utc>, _>(idx)
                .map(|dt| Value::Text(dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()))
        })
        .or_else(|_| {
            row.try_get::<chrono::NaiveTime, _>(idx)
                .map(|t| Value::Text(t.format("%H:%M:%S").to_string()))
        })
        .or_else(|_| row.try_get::<String, _>(idx).map(Value::Text))
        .or_else(|_| row.try_get::<Vec<u8>, _>(idx).map(Value::Blob))
        .unwrap_or(Value::Null)
}

fn row_to_values(row: &PgRow) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| value_from_row(row, i))
        .collect()
}

fn columns_from_row(row: &PgRow) -> Vec<ColumnInfo> {
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

fn parse_schema_table(identifier: &str) -> (String, String) {
    if let Some(dot_pos) = identifier.rfind('.') {
        let schema = &identifier[..dot_pos];
        let table = &identifier[dot_pos + 1..];
        (schema.to_string(), table.to_string())
    } else {
        ("public".to_string(), identifier.to_string())
    }
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
impl DatabaseProvider for PostgresProvider {
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
            "postgres://{}:{}@{}:{}/{}",
            encoded_user, encoded_pass, config.host, config.port, config.database,
        );

        tracing::debug!(
            "Connecting to PostgreSQL at {}:{}/{} as {}",
            config.host,
            config.port,
            config.database,
            config.username,
        );

        let pool = PgPoolOptions::new()
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
            || trimmed.starts_with("SHOW")
            || trimmed.starts_with("TABLE");

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
            "SELECT datname FROM pg_catalog.pg_database WHERE datistemplate = false ORDER BY datname",
        )
        .fetch_all(pool)
        .await?;

        let databases: Vec<String> = rows.iter().map(|r| r.get("datname")).collect();
        Ok(databases)
    }

    async fn get_schemas(&self) -> DatabaseResult<Vec<String>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT schema_name FROM information_schema.schemata \
             WHERE schema_name NOT IN ('pg_toast', 'pg_catalog', 'information_schema') \
             AND schema_name NOT LIKE 'pg_%' \
             ORDER BY schema_name",
        )
        .fetch_all(pool)
        .await?;

        let schemas: Vec<String> = rows.iter().map(|r| r.get("schema_name")).collect();
        Ok(schemas)
    }

    async fn get_tables(&self, schema: &str) -> DatabaseResult<Vec<TableInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema = $1 AND table_type IN ('BASE TABLE', 'VIEW') \
             ORDER BY table_name",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let tables = rows
            .iter()
            .map(|r| {
                let name: String = r.get("table_name");
                let table_type: String = r.get("table_type");
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
             c.ordinal_position::int AS ordinal, \
             c.column_name AS name, \
             c.udt_name AS type_name, \
             c.is_nullable, \
             CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END AS is_pk, \
             CASE WHEN fk.column_name IS NOT NULL THEN true ELSE false END AS is_fk, \
             c.column_default AS default_value, \
             CASE WHEN c.column_default LIKE 'nextval%%' THEN true ELSE false END AS is_auto_inc \
             FROM information_schema.columns c \
             LEFT JOIN ( \
             SELECT ku.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage ku \
             ON tc.constraint_name = ku.constraint_name \
             AND tc.table_schema = ku.table_schema \
             AND tc.table_name = ku.table_name \
             WHERE tc.constraint_type = 'PRIMARY KEY' \
             AND tc.table_schema = $1 \
             AND tc.table_name = $2 \
             ) pk ON c.column_name = pk.column_name \
             LEFT JOIN ( \
             SELECT ku.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage ku \
             ON tc.constraint_name = ku.constraint_name \
             AND tc.table_schema = ku.table_schema \
             AND tc.table_name = ku.table_name \
             WHERE tc.constraint_type = 'FOREIGN KEY' \
             AND tc.table_schema = $1 \
             AND tc.table_name = $2 \
             ) fk ON c.column_name = fk.column_name \
             WHERE c.table_schema = $1 AND c.table_name = $2 \
             ORDER BY c.ordinal_position",
        )
        .bind(&schema)
        .bind(&table_name)
        .fetch_all(pool)
        .await?;

        let columns = rows
            .iter()
            .map(|r| {
                let ordinal: i32 = r.get("ordinal");
                let name: String = r.get("name");
                let type_name: String = r.get("type_name");
                let nullable_str: String = r.get("is_nullable");
                let is_pk: bool = r.get("is_pk");
                let is_fk: bool = r.get("is_fk");
                let default_value: Option<String> = r.get("default_value");
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
            "SELECT \
             i.relname AS name, \
             ix.indisunique AS unique, \
             ix.indisprimary AS primary, \
             array_agg(a.attname ORDER BY a.attnum) AS columns \
             FROM pg_catalog.pg_class t \
             JOIN pg_catalog.pg_index ix ON t.oid = ix.indrelid \
             JOIN pg_catalog.pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace \
             JOIN pg_catalog.pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey) \
             WHERE t.relname = $2 AND n.nspname = $1 \
             AND a.attnum > 0 AND NOT a.attisdropped \
             GROUP BY i.relname, ix.indisunique, ix.indisprimary \
             ORDER BY i.relname",
        )
        .bind(&schema)
        .bind(&table_name)
        .fetch_all(pool)
        .await?;

        let indexes = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let columns: Vec<String> = r.get("columns");
                let unique: bool = r.get("unique");
                let primary: bool = r.get("primary");
                IndexInfo {
                    name,
                    columns,
                    unique,
                    primary,
                }
            })
            .collect();
        Ok(indexes)
    }

    async fn get_views(&self, schema: &str) -> DatabaseResult<Vec<ViewInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT table_name AS name, view_definition AS definition \
             FROM information_schema.views \
             WHERE table_schema = $1 \
             ORDER BY table_name",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let views = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let definition: Option<String> = r.get("definition");
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
            "SELECT sequence_name AS name, data_type, \
             start_value::bigint AS start_value, increment::bigint AS increment \
             FROM information_schema.sequences \
             WHERE sequence_schema = $1 \
             ORDER BY sequence_name",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let sequences = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let data_type: String = r.get("data_type");
                let start_value: Option<i64> = r.get("start_value");
                let increment: Option<i64> = r.get("increment");
                SequenceInfo {
                    name,
                    schema: schema.to_string(),
                    data_type,
                    start_value,
                    increment,
                }
            })
            .collect();
        Ok(sequences)
    }

    async fn get_triggers(&self, schema: &str) -> DatabaseResult<Vec<TriggerInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT \
             trigger_name AS name, \
             event_object_table AS table_name, \
             event_manipulation AS event, \
             action_timing AS timing, \
             action_orientation AS orientation, \
             action_statement AS definition \
             FROM information_schema.triggers \
             WHERE trigger_schema = $1 \
             ORDER BY trigger_name",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let triggers = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let table: String = r.get("table_name");
                let event: String = r.get("event");
                let timing: String = r.get("timing");
                let orientation: String = r.get("orientation");
                let definition: String = r.get("definition");
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
            "SELECT \
             p.proname AS name, \
             pg_catalog.pg_get_function_result(p.oid) AS return_type, \
             l.lanname AS language, \
             pg_catalog.pg_get_function_arguments(p.oid) AS args, \
             p.prosrc AS definition \
             FROM pg_catalog.pg_proc p \
             JOIN pg_catalog.pg_namespace n ON p.pronamespace = n.oid \
             JOIN pg_catalog.pg_language l ON p.prolang = l.oid \
             WHERE n.nspname = $1 \
             ORDER BY p.proname",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let functions = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let return_type: Option<String> = r.get("return_type");
                let language: String = r.get("language");
                let args_str: Option<String> = r.get("args");
                let definition: Option<String> = r.get("definition");

                let args = args_str
                    .as_ref()
                    .map(|a| {
                        if a.is_empty() {
                            Vec::new()
                        } else {
                            a.split(',')
                                .map(|s| {
                                    let trimmed = s.trim();
                                    if let Some(eq_pos) = trimmed.find("=> ") {
                                        let after_eq = &trimmed[eq_pos + 3..];
                                        let parts: Vec<&str> = after_eq.rsplitn(2, ' ').collect();
                                        let type_name =
                                            parts.first().unwrap_or(&trimmed).trim().to_string();
                                        ParamInfo {
                                            name: String::new(),
                                            type_name,
                                            mode: "IN".to_string(),
                                        }
                                    } else {
                                        let parts: Vec<&str> = trimmed.rsplitn(2, ' ').collect();
                                        if parts.len() == 2 {
                                            ParamInfo {
                                                name: parts[1].to_string(),
                                                type_name: parts[0].to_string(),
                                                mode: "IN".to_string(),
                                            }
                                        } else {
                                            ParamInfo {
                                                name: String::new(),
                                                type_name: trimmed.to_string(),
                                                mode: "IN".to_string(),
                                            }
                                        }
                                    }
                                })
                                .collect()
                        }
                    })
                    .unwrap_or_default();

                FunctionInfo {
                    name,
                    schema: schema.to_string(),
                    return_type,
                    language,
                    args,
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
            "SELECT \
             p.proname AS name, \
             pg_catalog.pg_get_function_arguments(p.oid) AS args \
             FROM pg_catalog.pg_proc p \
             JOIN pg_catalog.pg_namespace n ON p.pronamespace = n.oid \
             WHERE n.nspname = $1 AND p.prokind = 'p' \
             ORDER BY p.proname",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let procedures = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let args_str: Option<String> = r.get("args");
                let params = args_str
                    .as_ref()
                    .map(|a| {
                        if a.is_empty() {
                            Vec::new()
                        } else {
                            a.split(',')
                                .map(|s| {
                                    let parts: Vec<&str> = s.trim().rsplitn(2, ' ').collect();
                                    if parts.len() == 2 {
                                        ParamInfo {
                                            name: parts[1].to_string(),
                                            type_name: parts[0].to_string(),
                                            mode: "IN".to_string(),
                                        }
                                    } else {
                                        ParamInfo {
                                            name: String::new(),
                                            type_name: s.trim().to_string(),
                                            mode: "IN".to_string(),
                                        }
                                    }
                                })
                                .collect()
                        }
                    })
                    .unwrap_or_default();

                ProcedureInfo { name, params }
            })
            .collect();
        Ok(procedures)
    }

    async fn get_extensions(&self) -> DatabaseResult<Vec<String>> {
        let pool = self.pool()?;
        let rows = sqlx::query("SELECT extname FROM pg_catalog.pg_extension ORDER BY extname")
            .fetch_all(pool)
            .await?;

        let extensions: Vec<String> = rows.iter().map(|r| r.get("extname")).collect();
        Ok(extensions)
    }

    async fn get_custom_types(&self, schema: &str) -> DatabaseResult<Vec<TypeSchemaInfo>> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT \
             t.typname AS name, \
             CASE t.typtype \
             WHEN 'c' THEN 'composite' \
             WHEN 'e' THEN 'enum' \
             WHEN 'r' THEN 'range' \
             WHEN 'd' THEN 'domain' \
             ELSE 'other' END AS kind \
             FROM pg_catalog.pg_type t \
             JOIN pg_catalog.pg_namespace n ON t.typnamespace = n.oid \
             WHERE n.nspname = $1 \
             AND t.typtype IN ('c', 'e', 'r', 'd') \
             AND NOT EXISTS ( \
             SELECT 1 FROM pg_catalog.pg_class c \
             WHERE c.oid = t.typrelid AND c.relkind != 'c' \
             ) \
             ORDER BY t.typname",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;

        let types = rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let kind: String = r.get("kind");
                TypeSchemaInfo {
                    name,
                    schema: schema.to_string(),
                    kind,
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
        Ok(())
    }

    async fn server_info(&self) -> DatabaseResult<ServerInfo> {
        let pool = self.pool()?;

        let version_row = sqlx::query("SELECT version() AS version")
            .fetch_one(pool)
            .await?;
        let version: String = version_row.get("version");

        let uptime_row = sqlx::query(
            "SELECT (pg_catalog.now() - pg_catalog.pg_postmaster_start_time())::text AS uptime",
        )
        .fetch_one(pool)
        .await
        .ok();
        let uptime: Option<String> = uptime_row.and_then(|r| r.try_get("uptime").ok());

        Ok(ServerInfo {
            version,
            vendor: "PostgreSQL".to_string(),
            connection_uptime: uptime,
        })
    }
}
