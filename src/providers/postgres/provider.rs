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
                            tracing::debug!("No password found in keychain for {}", config.name);
                            String::new()
                        }
                        Err(e) => {
                            tracing::debug!("Keychain error for {}: {}", config.name, e);
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

    async fn get_create_table_ddl(&self, schema: &str, table: &str) -> DatabaseResult<String> {
        let pool = self.pool()?;

        // Get columns
        let col_rows = sqlx::query(
            r#"
            SELECT c.column_name, c.data_type, c.character_maximum_length,
                   c.is_nullable, c.column_default, c.numeric_precision, c.numeric_scale,
                   (CASE WHEN tc.constraint_type = 'PRIMARY KEY' THEN 1 ELSE 0 END) AS is_pk
            FROM information_schema.columns c
            LEFT JOIN information_schema.key_column_usage kcu
                ON c.table_schema = kcu.table_schema AND c.table_name = kcu.table_name AND c.column_name = kcu.column_name
            LEFT JOIN information_schema.table_constraints tc
                ON kcu.constraint_name = tc.constraint_name AND kcu.table_schema = tc.table_schema
                AND tc.constraint_type = 'PRIMARY KEY'
            WHERE c.table_schema = $1 AND c.table_name = $2
            ORDER BY c.ordinal_position
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await?;

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "CREATE TABLE {}.\"{}\" (",
            quote_pg_id(schema),
            table
        ));

        let mut col_parts: Vec<String> = Vec::new();
        for row in &col_rows {
            let col_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let char_max: Option<i32> = row.get("character_maximum_length");
            let num_prec: Option<i32> = row.get("numeric_precision");
            let num_scale: Option<i32> = row.get("numeric_scale");
            let is_nullable: String = row.get("is_nullable");
            let col_default: Option<String> = row.get("column_default");
            let is_pk: i32 = row.get("is_pk");

            let mut ct = data_type;
            if let Some(clen) = char_max {
                ct = format!("{}({})", ct, clen);
            } else if let Some(prec) = num_prec {
                if let Some(scale) = num_scale {
                    if scale > 0 {
                        ct = format!("{}({},{})", ct, prec, scale);
                    } else {
                        ct = format!("{}({})", ct, prec);
                    }
                }
            }

            let mut parts = vec![format!("    \"{}\" {}", col_name, ct)];
            if is_pk != 0 {
                // handled in constraints
            }
            if is_nullable == "NO" && is_pk == 0 {
                parts.push("NOT NULL".to_string());
            }
            if let Some(def) = col_default {
                parts.push(format!("DEFAULT {}", def));
            }
            col_parts.push(parts.join(" "));
        }

        // Get primary key constraint
        let pk_rows = sqlx::query(
            r#"
            SELECT kcu.column_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            WHERE tc.table_schema = $1 AND tc.table_name = $2 AND tc.constraint_type = 'PRIMARY KEY'
            ORDER BY kcu.ordinal_position
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await?;

        let pk_cols: Vec<String> = pk_rows.iter().map(|r| r.get("column_name")).collect();
        if !pk_cols.is_empty() {
            let cols: Vec<String> = pk_cols.iter().map(|c| format!("\"{}\"", c)).collect();
            col_parts.push(format!("    PRIMARY KEY ({})", cols.join(", ")));
        }

        // Get unique constraints
        let uq_rows = sqlx::query(
            r#"
            SELECT tc.constraint_name, kcu.column_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            WHERE tc.table_schema = $1 AND tc.table_name = $2 AND tc.constraint_type = 'UNIQUE'
            ORDER BY tc.constraint_name, kcu.ordinal_position
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await?;

        let mut uq_map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for r in &uq_rows {
            let cname: String = r.get("constraint_name");
            let col: String = r.get("column_name");
            uq_map.entry(cname).or_default().push(col);
        }
        for (_cname, cols) in &uq_map {
            let q: Vec<String> = cols.iter().map(|c| format!("\"{}\"", c)).collect();
            col_parts.push(format!("    UNIQUE ({})", q.join(", ")));
        }

        lines.push(col_parts.join(",\n"));
        lines.push(");".to_string());
        Ok(lines.join("\n"))
    }

    async fn get_create_view_ddl(&self, schema: &str, view: &str) -> DatabaseResult<String> {
        let pool = self.pool()?;
        let row = sqlx::query(
            "SELECT view_definition FROM information_schema.views WHERE table_schema = $1 AND table_name = $2",
        )
        .bind(schema)
        .bind(view)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DatabaseError::Metadata(format!("View \"{schema}.{view}\" not found")))?;
        let def: String = row.get("view_definition");
        Ok(format!(
            "CREATE VIEW {}.\"{}\" AS\n{};",
            quote_pg_id(schema),
            view,
            def
        ))
    }

    async fn get_full_schema_ddl(&self, schema: &str) -> DatabaseResult<String> {
        let pool = self.pool()?;

        let mut parts: Vec<String> = Vec::new();

        // Types
        if let Ok(types) = self.get_custom_types(schema).await {
            for t in types {
                if t.kind == "enum" {
                    // We don't have the enum values stored, so skip programmatic generation for now
                }
            }
        }

        // Sequences
        let seq_rows = sqlx::query(
            "SELECT sequence_name, data_type, start_value, increment FROM information_schema.sequences WHERE sequence_schema = $1",
        )
        .bind(schema)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for r in &seq_rows {
            let name: String = r.get("sequence_name");
            let dt: String = r.get("data_type");
            let sv: Option<i64> = r.get("start_value");
            let inc: Option<i64> = r.get("increment");
            let mut s = format!(
                "CREATE SEQUENCE {}.\"{}\" AS {}",
                quote_pg_id(schema),
                name,
                dt
            );
            if let Some(v) = sv {
                s.push_str(&format!(" START WITH {}", v));
            }
            if let Some(v) = inc {
                s.push_str(&format!(" INCREMENT BY {}", v));
            }
            s.push(';');
            parts.push(s);
        }

        // Tables
        let table_rows = sqlx::query(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = $1 AND table_type = 'BASE TABLE' ORDER BY table_name",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;
        for r in &table_rows {
            let tbl: String = r.get("table_name");
            if let Ok(ddl) = self.get_create_table_ddl(schema, &tbl).await {
                parts.push(ddl);
            }
        }

        // Indexes
        let idx_rows = sqlx::query(
            r#"
            SELECT indexname, indexdef FROM pg_catalog.pg_indexes WHERE schemaname = $1 AND tablename NOT LIKE '%pg_%' ORDER BY indexname
            "#,
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;
        for r in &idx_rows {
            let def: String = r.get("indexdef");
            let def = if !def.ends_with(';') {
                format!("{def};")
            } else {
                def
            };
            parts.push(def);
        }

        // Views
        let view_rows = sqlx::query(
            "SELECT table_name FROM information_schema.views WHERE table_schema = $1 ORDER BY table_name",
        )
        .bind(schema)
        .fetch_all(pool)
        .await?;
        for r in &view_rows {
            let vw: String = r.get("table_name");
            if let Ok(ddl) = self.get_create_view_ddl(schema, &vw).await {
                parts.push(ddl);
            }
        }

        // Triggers
        if let Ok(triggers) = self.get_triggers(schema).await {
            for t in triggers {
                parts.push(format!("{};", t.definition));
            }
        }

        // Functions
        if let Ok(funcs) = self.get_functions(schema).await {
            for f in funcs {
                if let Some(def) = f.definition {
                    parts.push(format!("{};", def));
                }
            }
        }

        parts.push(String::new());
        Ok(parts.join("\n\n"))
    }

    async fn get_foreign_keys(&self, table: &str) -> DatabaseResult<Vec<ForeignKeyInfo>> {
        let pool = self.pool()?;
        let (schema_name, table_name) = parse_schema_table(table);
        let rows = sqlx::query(
            r#"
            SELECT
                tc.constraint_name,
                kcu.column_name,
                ccu.table_name AS ref_table,
                ccu.column_name AS ref_column,
                rc.update_rule,
                rc.delete_rule
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage ccu
                ON ccu.constraint_name = tc.constraint_name AND ccu.table_schema = tc.table_schema
            JOIN information_schema.referential_constraints rc
                ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema
            WHERE tc.table_schema = $1 AND tc.table_name = $2 AND tc.constraint_type = 'FOREIGN KEY'
            ORDER BY tc.constraint_name, kcu.ordinal_position
            "#,
        )
        .bind(schema_name)
        .bind(table_name)
        .fetch_all(pool)
        .await?;

        let mut fk_map: std::collections::BTreeMap<String, ForeignKeyInfo> =
            std::collections::BTreeMap::new();
        for row in &rows {
            let cname: String = row.get("constraint_name");
            let col: String = row.get("column_name");
            let ref_tbl: String = row.get("ref_table");
            let ref_col: String = row.get("ref_column");
            let up_rule: String = row.get("update_rule");
            let del_rule: String = row.get("delete_rule");
            let entry = fk_map
                .entry(cname.clone())
                .or_insert_with(|| ForeignKeyInfo {
                    name: cname,
                    columns: Vec::new(),
                    ref_table: ref_tbl,
                    ref_columns: Vec::new(),
                    update_rule: Some(up_rule),
                    delete_rule: Some(del_rule),
                });
            entry.columns.push(col);
            entry.ref_columns.push(ref_col);
        }
        Ok(fk_map.into_values().collect())
    }
}

fn quote_pg_id(s: &str) -> String {
    format!("\"{}\"", s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_schema_table ─────────────────────────────────────

    #[test]
    fn parse_schema_table_with_schema() {
        assert_eq!(
            parse_schema_table("public.users"),
            ("public".to_string(), "users".to_string())
        );
    }

    #[test]
    fn parse_schema_table_no_schema() {
        assert_eq!(
            parse_schema_table("users"),
            ("public".to_string(), "users".to_string())
        );
    }

    #[test]
    fn parse_schema_table_custom_schema() {
        assert_eq!(
            parse_schema_table("my_schema.my_table"),
            ("my_schema".to_string(), "my_table".to_string())
        );
    }

    #[test]
    fn parse_schema_table_multiple_dots() {
        assert_eq!(
            parse_schema_table("a.b.c.table"),
            ("a.b.c".to_string(), "table".to_string())
        );
    }

    #[test]
    fn parse_schema_table_empty() {
        assert_eq!(
            parse_schema_table(""),
            ("public".to_string(), "".to_string())
        );
    }

    // ── encode_url_component ───────────────────────────────────

    #[test]
    fn encode_url_component_alphanumeric() {
        assert_eq!(encode_url_component("hello123"), "hello123");
    }

    #[test]
    fn encode_url_component_special_chars() {
        assert_eq!(encode_url_component("p@ss:word!"), "p%40ss%3Aword%21");
    }

    #[test]
    fn encode_url_component_safe_chars() {
        assert_eq!(encode_url_component("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn encode_url_component_space() {
        assert_eq!(encode_url_component("hello world"), "hello%20world");
    }

    #[test]
    fn encode_url_component_empty() {
        assert_eq!(encode_url_component(""), "");
    }

    #[test]
    fn encode_url_component_unicode() {
        // Non-ASCII chars get percent-encoded
        let encoded = encode_url_component("café");
        assert!(encoded.contains('%'));
        assert!(!encoded.contains('é'));
    }
}
