use std::time::Duration;

use async_trait::async_trait;
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use redis::aio::MultiplexedConnection;
use redis::Value as RedisValue;

use crate::database::provider::{DatabaseError, DatabaseProvider, DatabaseResult};
use crate::database::result::{QueryResult, Value};
use crate::database::types::*;
use crate::models::connection::ConnectionConfig;

const KEY_TYPES: &[&str] = &[
    "strings",
    "hashes",
    "lists",
    "sets",
    "sorted_sets",
    "streams",
];

fn category_to_redis_type(cat: &str) -> Option<&'static str> {
    match cat {
        "strings" => Some("string"),
        "hashes" => Some("hash"),
        "lists" => Some("list"),
        "sets" => Some("set"),
        "sorted_sets" => Some("zset"),
        "streams" => Some("stream"),
        _ => None,
    }
}

fn redis_value_to_string(rv: &RedisValue) -> String {
    match rv {
        RedisValue::Nil => "(nil)".to_string(),
        RedisValue::Int(i) => i.to_string(),
        RedisValue::BulkString(data) => String::from_utf8_lossy(data).to_string(),
        RedisValue::Array(items) => {
            let parts: Vec<String> = items.iter().map(redis_value_to_string).collect();
            format!("[{}]", parts.join(", "))
        }
        RedisValue::Okay => "OK".to_string(),
        RedisValue::SimpleString(s) => s.clone(),
        RedisValue::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", redis_value_to_string(k), redis_value_to_string(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        RedisValue::Set(items) => {
            let parts: Vec<String> = items.iter().map(redis_value_to_string).collect();
            format!("{{{}}}", parts.join(", "))
        }
        RedisValue::Double(f) => f.to_string(),
        RedisValue::Boolean(b) => b.to_string(),
        _ => format!("{:?}", rv),
    }
}

fn redis_value_to_value(rv: &RedisValue) -> Value {
    match rv {
        RedisValue::Nil => Value::Null,
        RedisValue::Int(i) => Value::Integer(*i),
        RedisValue::Okay => Value::Text("OK".to_string()),
        RedisValue::SimpleString(s) => Value::Text(s.clone()),
        RedisValue::BulkString(data) => {
            let s = String::from_utf8_lossy(data).to_string();
            if let Ok(n) = s.parse::<i64>() {
                Value::Integer(n)
            } else if let Ok(f) = s.parse::<f64>() {
                Value::Float(f)
            } else {
                Value::Text(s)
            }
        }
        RedisValue::Array(items) => {
            let parts: Vec<String> = items.iter().map(redis_value_to_string).collect();
            Value::Text(format!("[{}]", parts.join(", ")))
        }
        RedisValue::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", redis_value_to_string(k), redis_value_to_string(v)))
                .collect();
            Value::Text(format!("{{{}}}", parts.join(", ")))
        }
        RedisValue::Set(items) => {
            let parts: Vec<String> = items.iter().map(redis_value_to_string).collect();
            Value::Text(format!("{{{}}}", parts.join(", ")))
        }
        RedisValue::Double(f) => Value::Float(*f),
        RedisValue::Boolean(b) => Value::Bool(*b),
        _ => Value::Text(format!("{:?}", rv)),
    }
}

fn parse_redis_args(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
            }
            i += 1;
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' && i + 1 < chars.len() {
                i += 1;
                current.push(chars[i]);
            } else {
                current.push(c);
            }
            i += 1;
            continue;
        }
        if c == '\'' {
            in_single = true;
            i += 1;
            continue;
        }
        if c == '"' {
            in_double = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn parse_key_info(key: &str) -> (String, String) {
    if let Some(pos) = key.rfind('.') {
        let schema = &key[..pos];
        let name = &key[pos + 1..];
        (schema.to_string(), name.to_string())
    } else {
        ("db0".to_string(), key.to_string())
    }
}

pub struct RedisProvider {
    client: Option<redis::Client>,
    connection: Option<MultiplexedConnection>,
    db_index: String,
}

impl RedisProvider {
    pub fn new() -> Self {
        Self {
            client: None,
            connection: None,
            db_index: "0".to_string(),
        }
    }

    fn conn(&self) -> DatabaseResult<&MultiplexedConnection> {
        self.connection.as_ref().ok_or(DatabaseError::NotConnected)
    }

}

#[async_trait]
impl DatabaseProvider for RedisProvider {
    async fn connect(&mut self, config: &ConnectionConfig) -> DatabaseResult<()> {
        let pass: Option<String> = if config.password.as_deref().unwrap_or("").is_empty() {
            match crate::storage::credentials::get_password(&config.password_key()) {
                Ok(p) => p,
                Err(_) => None,
            }
        } else {
            config.password.clone()
        };
        let pw = pass.as_deref().unwrap_or("");
        let db = if config.database.is_empty() {
            "0"
        } else {
            &config.database
        };

        let url = if config.username.is_empty() && pw.is_empty() {
            format!("redis://{}:{}/{}", config.host, config.port, db)
        } else if config.username.is_empty() {
            let encoded: String = percent_encode(pw.as_bytes(), NON_ALPHANUMERIC).to_string();
            format!("redis://:{}@{}:{}/{}", encoded, config.host, config.port, db)
        } else {
            let encoded: String = percent_encode(pw.as_bytes(), NON_ALPHANUMERIC).to_string();
            format!(
                "redis://{}:{}@{}:{}/{}",
                config.username, encoded, config.host, config.port, db
            )
        };

        let client = redis::Client::open(url.as_str())
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;

        if db != "0" {
            let select_db: String = db.to_string();
            redis::cmd("SELECT")
                .arg(&select_db)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| {
                    DatabaseError::Connection(format!("Failed to select database {}: {}", db, e))
                })?;
        }

        self.client = Some(client);
        self.connection = Some(conn);
        self.db_index = db.to_string();
        Ok(())
    }

    async fn disconnect(&mut self) -> DatabaseResult<()> {
        self.connection = None;
        self.client = None;
        Ok(())
    }

    async fn ping(&self) -> DatabaseResult<bool> {
        let mut conn = self.conn()?.clone();
        let result: RedisValue = redis::cmd("PING")
            .query_async::<RedisValue>(&mut conn)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        Ok(matches!(result, RedisValue::SimpleString(s) if s == "PONG"))
    }

    async fn execute_query(&self, input: &str) -> DatabaseResult<QueryResult> {
        let trimmed = input.trim();
        let lines: Vec<&str> = trimmed
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            return Ok(QueryResult::new_execute(0, Duration::new(0, 0)));
        }
        let mut conn = self.conn()?.clone();
        let start = std::time::Instant::now();
        let mut last_result: Option<RedisValue> = None;
        let mut last_args: Vec<String> = Vec::new();

        for line in &lines {
            let args = parse_redis_args(line);
            if args.is_empty() {
                continue;
            }
            let mut cmd = redis::Cmd::new();
            cmd.arg(&args[0]);
            for arg in args.iter().skip(1) {
                cmd.arg(arg.as_str());
            }
            last_result = Some(
                cmd.query_async(&mut conn)
                    .await
                    .map_err(|e: redis::RedisError| DatabaseError::Execution(e.to_string()))?,
            );
            last_args = args;
        }

        let result = last_result.unwrap_or(RedisValue::Okay);
        let duration = start.elapsed();
        let args = &last_args;

        match &result {
            RedisValue::Nil => {
                let columns = vec![column_info("result", "string", 0)];
                let rows = vec![vec![Value::Text("(nil)".to_string())]];
                Ok(QueryResult::new_select(columns, rows, duration))
            }
            RedisValue::Okay => {
                let columns = vec![column_info("result", "string", 0)];
                let rows = vec![vec![Value::Text("OK".to_string())]];
                Ok(QueryResult::new_select(columns, rows, duration))
            }
            RedisValue::SimpleString(s) => {
                let columns = vec![column_info("result", "string", 0)];
                let rows = vec![vec![Value::Text(s.clone())]];
                Ok(QueryResult::new_select(columns, rows, duration))
            }
            RedisValue::Int(i) => {
                let command = args[0].to_uppercase();
                match command.as_str() {
                    "EXISTS" | "DEL" | "HDEL" | "SREM" | "ZREM" | "XDEL" | "APPEND" | "LPUSH"
                    | "RPUSH" | "SADD" | "ZADD" | "HSET" | "PUBLISH" | "RENAME" => {
                        Ok(QueryResult::new_execute(*i as u64, duration))
                    }
                    _ => {
                        let columns = vec![column_info("result", "integer", 0)];
                        let rows = vec![vec![Value::Integer(*i)]];
                        Ok(QueryResult::new_select(columns, rows, duration))
                    }
                }
            }
            RedisValue::BulkString(bytes) => {
                let val_str = String::from_utf8_lossy(bytes).to_string();
                let command = args.first().map(|s| s.to_uppercase());
                if command.as_deref() == Some("INFO") {
                    let mut parsed_rows: Vec<Vec<Value>> = Vec::new();
                    let columns = vec![
                        column_info("section", "string", 0),
                        column_info("key", "string", 1),
                        column_info("value", "string", 2),
                    ];
                    let mut current_section = "default".to_string();
                    for line in val_str.lines() {
                        if line.starts_with("# ") {
                            current_section = line[2..].to_string();
                            continue;
                        }
                        if let Some(eq_pos) = line.find('=') {
                            let k = line[..eq_pos].to_string();
                            let v = line[eq_pos + 1..].to_string();
                            parsed_rows.push(vec![
                                Value::Text(current_section.clone()),
                                Value::Text(k),
                                Value::Text(v),
                            ]);
                        }
                    }
                    if parsed_rows.is_empty() {
                        parsed_rows.push(vec![
                            Value::Text(val_str),
                            Value::Text(String::new()),
                            Value::Text(String::new()),
                        ]);
                    }
                    Ok(QueryResult::new_select(columns, parsed_rows, duration))
                } else {
                    let columns = vec![column_info("value", "string", 0)];
                    let rows = vec![vec![Value::Text(val_str)]];
                    Ok(QueryResult::new_select(columns, rows, duration))
                }
            }
            RedisValue::Array(items) => self.handle_array_result(items, &args, duration),
            RedisValue::Map(pairs) => {
                let mut parsed_rows: Vec<Vec<Value>> = Vec::new();
                let columns = vec![
                    column_info("field", "string", 0),
                    column_info("value", "string", 1),
                ];
                for (k, v) in pairs {
                    parsed_rows.push(vec![redis_value_to_value(k), redis_value_to_value(v)]);
                }
                Ok(QueryResult::new_select(columns, parsed_rows, duration))
            }
            RedisValue::Set(items) => {
                let columns = vec![column_info("value", "string", 0)];
                let rows: Vec<Vec<Value>> = items
                    .iter()
                    .map(|item| vec![redis_value_to_value(item)])
                    .collect();
                Ok(QueryResult::new_select(columns, rows, duration))
            }
            RedisValue::Double(f) => {
                let columns = vec![column_info("result", "float", 0)];
                let rows = vec![vec![Value::Float(*f)]];
                Ok(QueryResult::new_select(columns, rows, duration))
            }
            RedisValue::Boolean(b) => {
                let columns = vec![column_info("result", "boolean", 0)];
                let rows = vec![vec![Value::Bool(*b)]];
                Ok(QueryResult::new_select(columns, rows, duration))
            }
            _ => {
                let columns = vec![column_info("result", "string", 0)];
                let rows = vec![vec![Value::Text(format!("{:?}", result))]];
                Ok(QueryResult::new_select(columns, rows, duration))
            }
        }
    }

    async fn get_databases(&self) -> DatabaseResult<Vec<String>> {
        Ok((0..16).map(|n| format!("db{}", n)).collect())
    }

    async fn get_schemas(&self) -> DatabaseResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn get_tables(&self, schema: &str) -> DatabaseResult<Vec<TableInfo>> {
        let tables: Vec<TableInfo> = KEY_TYPES
            .iter()
            .map(|t| TableInfo {
                name: t.to_string(),
                schema: schema.to_string(),
                table_type: "REDIS_TYPE".to_string(),
            })
            .collect();
        Ok(tables)
    }

    async fn get_columns(&self, table: &str) -> DatabaseResult<Vec<ColumnInfo>> {
        let (schema, type_name) = parse_key_info(table);
        let Some(redis_type) = category_to_redis_type(&type_name) else {
            return Ok(Vec::new());
        };
        let mut conn = self.conn()?.clone();

        let db_num = schema.trim_start_matches("db");
        if db_num.is_empty() {
            return Err(DatabaseError::Metadata(format!(
                "invalid database schema: {}",
                schema
            )));
        }
        redis::cmd("SELECT")
            .arg(db_num)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?;

        let mut cursor: i64 = 0;
        let mut columns: Vec<ColumnInfo> = Vec::new();
        let limit = 1000i64;

        loop {
            let mut scan_cmd = redis::Cmd::new();
            scan_cmd.arg("SCAN");
            scan_cmd.arg(cursor);
            scan_cmd.arg("TYPE");
            scan_cmd.arg(redis_type);
            scan_cmd.arg("COUNT");
            scan_cmd.arg(limit);

            let scan_result: RedisValue = scan_cmd
                .query_async(&mut conn)
                .await
                .map_err(|e| DatabaseError::Metadata(e.to_string()))?;

            match scan_result {
                RedisValue::Array(ref parts) if parts.len() >= 2 => {
                    cursor = match &parts[0] {
                        RedisValue::Int(i) => *i,
                        RedisValue::BulkString(d) => {
                            String::from_utf8_lossy(d).parse().unwrap_or(0)
                        }
                        _ => 0,
                    };
                    if let RedisValue::Array(ref keys) = parts[1] {
                        for (idx, key_item) in keys.iter().enumerate() {
                            let key_name = redis_value_to_string(key_item);
                            let mut ttl_cmd = redis::Cmd::new();
                            ttl_cmd.arg("TTL");
                            ttl_cmd.arg(&key_name);
                            let ttl: RedisValue = ttl_cmd
                                .query_async(&mut conn)
                                .await
                                .unwrap_or(RedisValue::Int(-2));
                            let ttl_val = match ttl {
                                RedisValue::Int(i) => i,
                                _ => -2,
                            };
                            columns.push(ColumnInfo {
                                name: key_name,
                                ordinal: columns.len() + idx,
                                type_name: if ttl_val < 0 {
                                    type_name.clone()
                                } else {
                                    format!("{} ttl:{}s", type_name, ttl_val)
                                },
                                nullable: false,
                                is_pk: true,
                                is_fk: false,
                                default: None,
                                is_auto_increment: false,
                            });
                        }
                    }
                }
                _ => break,
            }

            if cursor == 0 {
                break;
            }
        }

        Ok(columns)
    }

    async fn get_indexes(&self, _table: &str) -> DatabaseResult<Vec<IndexInfo>> {
        Ok(Vec::new())
    }

    async fn get_views(&self, _schema: &str) -> DatabaseResult<Vec<ViewInfo>> {
        Ok(Vec::new())
    }

    async fn get_procedures(&self, _schema: &str) -> DatabaseResult<Vec<ProcedureInfo>> {
        Ok(Vec::new())
    }

    async fn begin_transaction(&self) -> DatabaseResult<()> {
        Err(DatabaseError::Transaction(
            "Redis transactions not supported via this interface. Use MULTI/EXEC in the query editor."
                .to_string(),
        ))
    }

    async fn commit(&self) -> DatabaseResult<()> {
        Err(DatabaseError::Transaction(
            "Redis transactions not supported via this interface. Use MULTI/EXEC in the query editor."
                .to_string(),
        ))
    }

    async fn rollback(&self) -> DatabaseResult<()> {
        Err(DatabaseError::Transaction(
            "Redis transactions not supported via this interface. Use MULTI/EXEC in the query editor."
                .to_string(),
        ))
    }

    async fn cancel_query(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn server_info(&self) -> DatabaseResult<ServerInfo> {
        let mut conn = self.conn()?.clone();
        let mut info_cmd = redis::Cmd::new();
        info_cmd.arg("INFO");
        info_cmd.arg("server");
        let info_str: String = info_cmd
            .query_async(&mut conn)
            .await
            .map_err(|e: redis::RedisError| DatabaseError::Metadata(e.to_string()))?;

        let mut version = String::new();
        let mut uptime = None;
        for line in info_str.lines() {
            if let Some(v) = line.strip_prefix("redis_version:") {
                version = v.trim().to_string();
            }
            if let Some(u) = line.strip_prefix("uptime_in_seconds:") {
                let secs: u64 = u.trim().parse().unwrap_or(0);
                uptime = Some(format!("{}s", secs));
            }
        }

        Ok(ServerInfo {
            vendor: "Redis".to_string(),
            version: format!("Redis v{}", if version.is_empty() { "?" } else { &version }),
            connection_uptime: uptime,
        })
    }
}

impl RedisProvider {
    fn handle_array_result(
        &self,
        items: &[RedisValue],
        args: &[String],
        duration: Duration,
    ) -> DatabaseResult<QueryResult> {
        let mut parsed_rows: Vec<Vec<Value>> = Vec::new();
        let columns: Vec<ColumnInfo>;

        if items.is_empty() {
            columns = vec![column_info("result", "string", 0)];
            return Ok(QueryResult::new_select(columns, vec![], duration));
        }

        let command = if args.is_empty() {
            ""
        } else {
            &args[0].to_uppercase()
        };

        match command {
            "HGETALL" => {
                columns = vec![
                    column_info("field", "string", 0),
                    column_info("value", "string", 1),
                ];
                let mut i = 0;
                while i + 1 < items.len() {
                    let field = redis_value_to_value(&items[i]);
                    let val = redis_value_to_value(&items[i + 1]);
                    parsed_rows.push(vec![field, val]);
                    i += 2;
                }
            }
            "LRANGE" | "SMEMBERS" | "SUBSCRIBE" | "PSUBSCRIBE" => {
                columns = vec![column_info("value", "string", 0)];
                for item in items {
                    parsed_rows.push(vec![redis_value_to_value(item)]);
                }
            }
            "ZRANGE" | "ZREVRANGE" | "ZRANGEBYSCORE" | "ZREVRANGEBYSCORE" => {
                let has_scores = args.iter().any(|a| a.to_uppercase() == "WITHSCORES");
                if has_scores && items.len() >= 2 {
                    columns = vec![
                        column_info("member", "string", 0),
                        column_info("score", "string", 1),
                    ];
                    let mut i = 0;
                    while i + 1 < items.len() {
                        let member = redis_value_to_value(&items[i]);
                        let score = redis_value_to_value(&items[i + 1]);
                        parsed_rows.push(vec![member, score]);
                        i += 2;
                    }
                } else {
                    columns = vec![column_info("member", "string", 0)];
                    for item in items {
                        parsed_rows.push(vec![redis_value_to_value(item)]);
                    }
                }
            }
            "SCAN" | "SSCAN" | "HSCAN" | "ZSCAN" => {
                if items.len() >= 2 {
                    let cursor = redis_value_to_string(&items[0]);
                    let keys = if let RedisValue::Array(ref key_items) = items[1] {
                        key_items
                            .iter()
                            .map(redis_value_to_string)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    columns = vec![
                        column_info("cursor", "string", 0),
                        column_info("key", "string", 1),
                    ];
                    parsed_rows.push(vec![Value::Text(cursor), Value::Text(keys.join(", "))]);
                    for key in &keys {
                        parsed_rows
                            .push(vec![Value::Text("".to_string()), Value::Text(key.clone())]);
                    }
                } else {
                    return self.fallback_single_column(items, duration);
                }
            }
            "XRANGE" | "XREVRANGE" | "XREAD" => {
                columns = vec![
                    column_info("id", "string", 0),
                    column_info("field", "string", 1),
                    column_info("value", "string", 2),
                ];
                for item in items {
                    if let RedisValue::Array(parts) = item {
                        if parts.len() >= 2 {
                            let entry_id = redis_value_to_string(&parts[0]);
                            if let RedisValue::Array(fields) = &parts[1] {
                                let mut fi = 0;
                                while fi + 1 < fields.len() {
                                    parsed_rows.push(vec![
                                        Value::Text(entry_id.clone()),
                                        redis_value_to_value(&fields[fi]),
                                        redis_value_to_value(&fields[fi + 1]),
                                    ]);
                                    fi += 2;
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                if items.len() > 1
                    && items.len() % 2 == 0
                    && items.iter().step_by(2).all(|v| {
                        matches!(v, RedisValue::BulkString(_) | RedisValue::SimpleString(_))
                    })
                {
                    columns = vec![
                        column_info("field", "string", 0),
                        column_info("value", "string", 1),
                    ];
                    let mut i = 0;
                    while i + 1 < items.len() {
                        parsed_rows.push(vec![
                            redis_value_to_value(&items[i]),
                            redis_value_to_value(&items[i + 1]),
                        ]);
                        i += 2;
                    }
                } else {
                    return self.fallback_single_column(items, duration);
                }
            }
        }

        Ok(QueryResult::new_select(columns, parsed_rows, duration))
    }

    fn fallback_single_column(
        &self,
        items: &[RedisValue],
        duration: Duration,
    ) -> DatabaseResult<QueryResult> {
        let columns = vec![column_info("value", "string", 0)];
        let rows: Vec<Vec<Value>> = items
            .iter()
            .map(|v| vec![redis_value_to_value(v)])
            .collect();
        Ok(QueryResult::new_select(columns, rows, duration))
    }
}

fn column_info(name: &str, type_name: &str, ordinal: usize) -> ColumnInfo {
    ColumnInfo {
        name: name.to_string(),
        ordinal,
        type_name: type_name.to_string(),
        nullable: false,
        is_pk: false,
        is_fk: false,
        default: None,
        is_auto_increment: false,
    }
}
