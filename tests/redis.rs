/// Integration test for the Redis provider.
///
/// Requires a Redis instance running on 127.0.0.1:6379 (no auth).
/// Override with REDIS_HOST and REDIS_PORT env vars.
use dbrusty::database::provider::DatabaseProvider;
use dbrusty::models::connection::{ConnectionConfig, DatabaseType};
use dbrusty::providers::redis::RedisProvider;

fn get_config() -> Option<ConnectionConfig> {
    let host = std::env::var("REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("REDIS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6379);
    Some(ConnectionConfig {
        id: uuid::Uuid::new_v4(),
        name: "test-redis".into(),
        db_type: DatabaseType::Redis,
        host,
        port,
        database: String::new(),
        username: String::new(),
        group: None,
        is_favorite: false,
        ssl: false,
        created_at: chrono::Utc::now(),
        last_connected: None,
        password: None,
    })
}

fn key_name(prefix: &str) -> String {
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    format!("test:{}:{}", prefix, suffix)
}

async fn ensure_provider(config: &ConnectionConfig) -> RedisProvider {
    let mut provider = RedisProvider::new();
    provider.connect(config).await.unwrap();
    provider
}

// ── Connection lifecycle ─────────────────────────────────────────

#[tokio::test]
async fn test_redis_connect_and_ping() {
    let mut provider = RedisProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    assert!(provider.ping().await.unwrap());
}

#[tokio::test]
async fn test_redis_server_info() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let info = provider.server_info().await.unwrap();
    assert_eq!(info.vendor, "Redis");
    assert!(!info.version.is_empty());
}

#[tokio::test]
async fn test_redis_disconnect() {
    let mut provider = RedisProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    provider.disconnect().await.unwrap();
    assert!(provider.ping().await.is_err());
}

// ── Metadata ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_redis_get_databases() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let databases = provider.get_databases().await.unwrap();
    assert_eq!(databases.len(), 16, "should return db0..db15");
    for i in 0..16 {
        assert!(
            databases.contains(&format!("db{}", i)),
            "should contain db{}",
            i
        );
    }
}

#[tokio::test]
async fn test_redis_get_schemas() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let schemas = provider.get_schemas().await.unwrap();
    assert!(schemas.is_empty(), "Redis has no schemas");
}

#[tokio::test]
async fn test_redis_get_tables() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let tables = provider.get_tables("db0").await.unwrap();
    assert_eq!(tables.len(), 6, "should return 6 key types");
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"strings"), "should contain strings");
    assert!(names.contains(&"hashes"), "should contain hashes");
    assert!(names.contains(&"lists"), "should contain lists");
    assert!(names.contains(&"sets"), "should contain sets");
    assert!(names.contains(&"sorted_sets"), "should contain sorted_sets");
    assert!(names.contains(&"streams"), "should contain streams");
    for t in &tables {
        assert_eq!(t.schema, "db0", "all tables should have schema 'db0'");
        assert_eq!(t.table_type, "REDIS_TYPE");
    }
}

#[tokio::test]
async fn test_redis_get_columns_strings() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("columns_strings");
    provider
        .execute_query(&format!("SET {} hello", k))
        .await
        .unwrap();

    let columns = provider
        .get_columns(&format!("db0.strings"))
        .await
        .unwrap();
    assert!(
        columns.iter().any(|c| c.name == k),
        "should find our string key"
    );
    let col = columns.iter().find(|c| c.name == k).unwrap();
    assert_eq!(col.is_pk, true, "Redis keys should have is_pk=true");
    assert!(col.type_name.contains("string"), "type should be string");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_get_columns_empty_type() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let columns = provider
        .get_columns("db0.nonexistent_type")
        .await
        .unwrap();
    assert!(columns.is_empty(), "unknown type should return empty list");
}

#[tokio::test]
async fn test_redis_get_indexes() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let indexes = provider.get_indexes("db0.strings").await.unwrap();
    assert!(indexes.is_empty(), "Redis has no index metadata");
}

#[tokio::test]
async fn test_redis_get_views() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let views = provider.get_views("db0").await.unwrap();
    assert!(views.is_empty(), "Redis has no views");
}

#[tokio::test]
async fn test_redis_get_procedures() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let procedures = provider.get_procedures("db0").await.unwrap();
    assert!(procedures.is_empty(), "Redis has no stored procedures");
}

// ── Query execution: strings ─────────────────────────────────────

#[tokio::test]
async fn test_redis_set_get() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("set_get");

    let set_result = provider
        .execute_query(&format!("SET {} hello", k))
        .await
        .unwrap();
    assert!(set_result.is_select, "SET should be is_select");
    assert_eq!(set_result.row_count(), 1);
    assert_eq!(set_result.rows[0][0].to_string(), "OK");

    let get_result = provider
        .execute_query(&format!("GET {}", k))
        .await
        .unwrap();
    assert!(get_result.is_select, "GET should be is_select");
    assert_eq!(get_result.row_count(), 1);
    assert!(get_result.rows[0][0].to_string().contains("hello"));

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_get_nonexistent() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let result = provider
        .execute_query(&format!("GET {}", key_name("nonexistent")))
        .await
        .unwrap();
    assert!(result.is_select);
    assert_eq!(result.row_count(), 1);
    assert_eq!(result.rows[0][0].to_string(), "(nil)");
}

#[tokio::test]
async fn test_redis_append() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("append");

    provider.execute_query(&format!("SET {} hello", k)).await.unwrap();
    let r = provider
        .execute_query(&format!("APPEND {} world", k))
        .await
        .unwrap();
    assert!(!r.is_select, "APPEND should be execute");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Query execution: hashes ──────────────────────────────────────

#[tokio::test]
async fn test_redis_hset_hgetall() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("hash");

    provider
        .execute_query(&format!("HSET {} name alice", k))
        .await
        .unwrap();
    provider
        .execute_query(&format!("HSET {} age 30", k))
        .await
        .unwrap();

    let r = provider.execute_query(&format!("HGETALL {}", k)).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.col_count(), 2, "HGETALL should have 2 columns (field, value)");
    assert_eq!(r.row_count(), 2, "should have 2 rows for name and age");
    let names: Vec<String> = r.rows.iter().map(|row| row[0].to_string()).collect();
    assert!(names.contains(&"name".to_string()), "should contain 'name' field");
    assert!(names.contains(&"age".to_string()), "should contain 'age' field");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_hget() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("hget");

    provider
        .execute_query(&format!("HSET {} field value", k))
        .await
        .unwrap();
    let r = provider.execute_query(&format!("HGET {} field", k)).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.rows[0][0].to_string(), "value");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_hdel() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("hdel");

    provider
        .execute_query(&format!("HSET {} field1 v1 field2 v2", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("HDEL {} field1", k))
        .await
        .unwrap();
    assert!(!r.is_select, "HDEL should be execute");
    assert_eq!(r.rows_affected, 1, "HDEL should affect 1 row");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Query execution: lists ───────────────────────────────────────

#[tokio::test]
async fn test_redis_lpush_lrange() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("list");

    let r = provider
        .execute_query(&format!("LPUSH {} a b c", k))
        .await
        .unwrap();
    assert!(!r.is_select, "LPUSH should be execute");
    assert_eq!(r.rows_affected, 3, "LPUSH should affect 3 rows");

    let r = provider
        .execute_query(&format!("LRANGE {} 0 -1", k))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 3, "LRANGE should return 3 items");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_llen() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("llen");

    provider
        .execute_query(&format!("LPUSH {} x y z", k))
        .await
        .unwrap();
    let r = provider.execute_query(&format!("LLEN {}", k)).await.unwrap();
    assert!(r.is_select, "LLEN should be select");
    assert_eq!(r.rows[0][0].to_string(), "3");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Query execution: sets ────────────────────────────────────────

#[tokio::test]
async fn test_redis_sadd_smembers() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("set");

    let r = provider
        .execute_query(&format!("SADD {} alice bob charlie", k))
        .await
        .unwrap();
    assert!(!r.is_select, "SADD should be execute");
    assert_eq!(r.rows_affected, 3);

    let r = provider.execute_query(&format!("SMEMBERS {}", k)).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 3, "SMEMBERS should return 3 members");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_srem() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("srem");

    provider
        .execute_query(&format!("SADD {} a b c", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("SREM {} a", k))
        .await
        .unwrap();
    assert!(!r.is_select, "SREM should be execute");
    assert_eq!(r.rows_affected, 1);

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Query execution: sorted sets ─────────────────────────────────

#[tokio::test]
async fn test_redis_zadd_zrange() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("zset");

    let r = provider
        .execute_query(&format!("ZADD {} 1 one 2 two 3 three", k))
        .await
        .unwrap();
    assert!(!r.is_select, "ZADD should be execute");
    assert_eq!(r.rows_affected, 3);

    let r = provider
        .execute_query(&format!("ZRANGE {} 0 -1", k))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 3, "ZRANGE should return 3 members");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_zrange_withscores() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("zset_ws");

    provider
        .execute_query(&format!("ZADD {} 10 alpha 20 beta", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("ZRANGE {} 0 -1 WITHSCORES", k))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.col_count(), 2, "WITHSCORES should have 2 columns");
    assert_eq!(r.row_count(), 2, "WITHSCORES should return 2 pairs");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_zrem() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("zrem");

    provider
        .execute_query(&format!("ZADD {} 1 a 2 b", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("ZREM {} a", k))
        .await
        .unwrap();
    assert!(!r.is_select, "ZREM should be execute");
    assert_eq!(r.rows_affected, 1);

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Query execution: streams ─────────────────────────────────────

#[tokio::test]
async fn test_redis_xadd_xrange() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("stream");

    let r = provider
        .execute_query(&format!("XADD {} * name alice age 30", k))
        .await
        .unwrap();
    assert!(r.is_select, "XADD should be select (returns id)");
    assert_eq!(r.row_count(), 1, "XADD should return 1 row");
    let entry_id = r.rows[0][0].to_string();
    assert!(!entry_id.is_empty(), "XADD should return an entry id");

    let r = provider
        .execute_query(&format!("XRANGE {} - +", k))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.col_count(), 3, "XRANGE should have 3 columns");
    assert_eq!(r.row_count(), 2, "XRANGE should return 2 field-value pairs");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_xdel() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("xdel");

    let r = provider
        .execute_query(&format!("XADD {} * temp val", k))
        .await
        .unwrap();
    let entry_id = r.rows[0][0].to_string();
    let r = provider
        .execute_query(&format!("XDEL {} {}", k, entry_id))
        .await
        .unwrap();
    assert!(!r.is_select, "XDEL should be execute");
    assert_eq!(r.rows_affected, 1);

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Query execution: general ─────────────────────────────────────

#[tokio::test]
async fn test_redis_del_execute() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("del_test");

    provider
        .execute_query(&format!("SET {} val", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
    assert!(!r.is_select, "DEL should be execute");
    assert_eq!(r.rows_affected, 1, "DEL should affect 1 row");
}

#[tokio::test]
async fn test_redis_exists() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("exists_test");

    provider
        .execute_query(&format!("SET {} val", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("EXISTS {}", k))
        .await
        .unwrap();
    assert!(!r.is_select, "EXISTS should be execute");
    assert_eq!(r.rows_affected, 1, "EXISTS should show key exists");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("EXISTS {}", k))
        .await
        .unwrap();
    assert!(!r.is_select, "EXISTS should be execute");
    assert_eq!(r.rows_affected, 0, "EXISTS should show key gone");
}

#[tokio::test]
async fn test_redis_incr() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("counter");

    provider.execute_query(&format!("SET {} 0", k)).await.unwrap();
    let r = provider
        .execute_query(&format!("INCR {}", k))
        .await
        .unwrap();
    assert!(r.is_select, "INCR should be select (not in execute list)");
    assert_eq!(r.rows[0][0].to_string(), "1");

    let r = provider
        .execute_query(&format!("INCR {}", k))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "2");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_ping() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.execute_query("PING").await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][0].to_string(), "PONG");
}

#[tokio::test]
async fn test_redis_select() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.execute_query("SELECT 0").await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.rows[0][0].to_string(), "OK");
}

#[tokio::test]
async fn test_redis_info() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.execute_query("INFO server").await.unwrap();
    assert!(r.is_select, "INFO should be select");
    assert_eq!(r.col_count(), 3, "INFO should have 3 columns");
    assert!(!r.rows.is_empty(), "INFO should return rows");
}

#[tokio::test]
async fn test_redis_empty_input() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.execute_query("").await.unwrap();
    assert!(!r.is_select, "empty input should be execute");
    assert_eq!(r.rows_affected, 0);
}

#[tokio::test]
async fn test_redis_multi_line() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("multi_line");

    let input = format!("SET {} hello\nGET {}", k, k);
    let r = provider.execute_query(&input).await.unwrap();
    assert!(r.is_select, "multi-line ending with GET should be select");
    assert_eq!(r.row_count(), 1);
    assert!(r.rows[0][0].to_string().contains("hello"));

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_type() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k = key_name("type_test");

    provider
        .execute_query(&format!("SET {} str", k))
        .await
        .unwrap();
    let r = provider.execute_query(&format!("TYPE {}", k)).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.rows[0][0].to_string(), "string");

    provider
        .execute_query(&format!("DEL {}", k))
        .await
        .unwrap();
}

// ── Error handling ───────────────────────────────────────────────

#[tokio::test]
async fn test_redis_invalid_command() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.execute_query("NOTACOMMAND").await;
    assert!(r.is_err(), "invalid command should error");
}

#[tokio::test]
async fn test_redis_not_connected() {
    let provider = RedisProvider::new();
    let r = provider.execute_query("PING").await;
    assert!(r.is_err(), "should error when not connected");
    match r {
        Err(dbrusty::database::provider::DatabaseError::NotConnected) => {}
        _ => panic!("expected NotConnected error"),
    }
}

// ── Transactions ─────────────────────────────────────────────────

#[tokio::test]
async fn test_redis_begin_transaction_error() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.begin_transaction().await;
    assert!(r.is_err(), "Redis should not support transactions");
}

#[tokio::test]
async fn test_redis_commit_error() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.commit().await;
    assert!(r.is_err(), "Redis should not support commit");
}

#[tokio::test]
async fn test_redis_rollback_error() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.rollback().await;
    assert!(r.is_err(), "Redis should not support rollback");
}

// ── Edge cases ───────────────────────────────────────────────────

#[tokio::test]
async fn test_redis_rename() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let k_src = key_name("rename_src");
    let k_dst = key_name("rename_dst");

    provider
        .execute_query(&format!("SET {} val", k_src))
        .await
        .unwrap();
    let r = provider
        .execute_query(&format!("RENAME {} {}", k_src, k_dst))
        .await
        .unwrap();
    assert!(r.is_select, "RENAME should be select (returns OK)");
    assert_eq!(r.rows[0][0].to_string(), "OK");

    provider
        .execute_query(&format!("DEL {}", k_dst))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_cancel_query() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.cancel_query().await;
    assert!(r.is_ok(), "cancel_query should always succeed");
}

#[tokio::test]
async fn test_redis_get_columns_invalid_schema() {
    let provider = ensure_provider(&get_config().unwrap()).await;
    let r = provider.get_columns("invalid_schema.strings").await;
    assert!(r.is_err(), "invalid schema should error");
}
