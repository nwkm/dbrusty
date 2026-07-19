/// Integration test for the MongoDB provider.
///
/// Requires a MongoDB instance running on 127.0.0.1:27017 (no auth).
/// Override with MONGODB_HOST, MONGODB_PORT, MONGODB_DATABASE env vars.

use dbrusty::database::provider::DatabaseProvider;
use dbrusty::database::result::Value;
use dbrusty::models::connection::{ConnectionConfig, DatabaseType};
use dbrusty::providers::mongodb::MongoProvider;

fn get_config() -> Option<ConnectionConfig> {
    let host = std::env::var("MONGODB_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("MONGODB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(27017);
    let database = std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "test".into());

    Some(ConnectionConfig {
        id: uuid::Uuid::new_v4(),
        name: "test-mongodb".into(),
        db_type: DatabaseType::MongoDB,
        host,
        port,
        database,
        username: String::new(),
        group: None,
        is_favorite: false,
        ssl: false,
        created_at: chrono::Utc::now(),
        last_connected: None,
        password: None,
    })
}

fn coll_name(prefix: &str) -> String {
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    format!("test_{}_{}", prefix, suffix)
}

async fn clear_collection(provider: &MongoProvider, name: &str) {
    let q = format!(r#"{{"deleteMany": "{}", "filter": {{}}}}"#, name);
    let _ = provider.execute_query(&q).await;
}

// ── Connection tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_mongodb_connect_and_ping() {
    let mut provider = MongoProvider::new();
    let config = get_config().unwrap();
    let r = provider.connect(&config).await;
    assert!(r.is_ok(), "connect failed: {:?}", r.err());
    assert!(provider.ping().await.unwrap());
}

#[tokio::test]
async fn test_mongodb_get_databases() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let databases = provider.get_databases().await.unwrap();
    for db_name in &databases {
        assert!(!db_name.is_empty());
    }
}

#[tokio::test]
async fn test_mongodb_server_info() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let info = provider.server_info().await.unwrap();
    assert_eq!(info.vendor, "MongoDB");
    assert!(!info.version.is_empty());
}

#[tokio::test]
async fn test_mongodb_disconnect() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    provider.disconnect().await.unwrap();
    assert!(provider.ping().await.is_err());
}

// ── Query execution tests ─────────────────────────────────────────

#[tokio::test]
async fn test_mongodb_insert_one() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("insert_one");

    let q = format!(r#"{{"insertOne": "{}", "document": {{"name": "alice", "age": 30}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select, "insertOne should not be select");
    assert_eq!(r.rows_affected, 1, "should affect 1 row");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_insert_many() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("insert_many");

    let q = format!(
        r#"{{"insertMany": "{}", "documents": [{{"x": 1}}, {{"x": 2}}, {{"x": 3}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 3);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_find_all() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("find_all");

    let insert = format!(r#"{{"insertMany": "{}", "documents": [{{"v": 1}}, {{"v": 2}}]}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"find": "{}"}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 2, "should return 2 documents");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_find_with_filter() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("find_filter");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"val": "a"}}, {{"val": "b"}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"find": "{}", "filter": {{"val": "a"}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_find_with_limit() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("find_limit");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"n": 1}}, {{"n": 2}}, {{"n": 3}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"find": "{}", "limit": 2}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 2);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_update_one() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("update_one");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"item": "x", "status": "old"}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{"item": "x"}}, "update": {{"$set": {{"status": "new"}}}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_update_many() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("update_many");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"g": 1}}, {{"g": 1}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"updateMany": "{}", "filter": {{"g": 1}}, "update": {{"$set": {{"g": 2}}}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 2);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_delete_one() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("delete_one");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"k": 1}}, {{"k": 2}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"deleteOne": "{}", "filter": {{"k": 1}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_delete_many() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("delete_many");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"z": 1}}, {{"z": 2}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"deleteMany": "{}", "filter": {{}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 2);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_count() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("count");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"a": 1}}, {{"a": 2}}, {{"a": 3}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"count": "{}", "filter": {{"a": {{"$gt": 1}}}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);

    if let Some(Value::Integer(n)) = r.rows.get(0).and_then(|row| row.get(0)) {
        assert_eq!(*n, 2);
    } else {
        panic!("expected Integer count");
    }

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_distinct() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("distinct");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"color": "red"}}, {{"color": "blue"}}, {{"color": "red"}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"distinct": "{}", "field": "color", "filter": {{}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 2);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("aggregate");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"city": "NYC", "pop": 1000}}, {{"city": "LA", "pop": 2000}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$group": {{"_id": null, "total": {{"$sum": "$pop"}}}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_coll_stats() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("coll_stats");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"dummy": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"collStats": "{}"}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert!(r.row_count() >= 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_explain() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("explain");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"x": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"explain": "{}", "filter": {{"x": 1}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert!(r.row_count() >= 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_shorthand_query() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("shorthand");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"val": 42}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"{}": 1}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_get_tables() {
    let config = get_config().unwrap();
    let db_name = config.database.clone();
    let mut provider = MongoProvider::new();
    provider.connect(&config).await.unwrap();
    let coll = coll_name("get_tables");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"tmp": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let tables = provider.get_tables(&db_name).await.unwrap();
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.iter().any(|n| *n == coll), "collection should appear in get_tables");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_get_columns() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("get_columns");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"name": "alice", "age": 30, "active": true}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let columns = provider.get_columns(&coll).await.unwrap();
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"_id"), "should have _id column");
    assert!(names.contains(&"name"), "should have name column");
    assert!(names.contains(&"age"), "should have age column");
    assert!(names.contains(&"active"), "should have active column");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_get_indexes() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("get_indexes");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"i": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    let indexes = provider.get_indexes(&coll).await.unwrap();
    assert!(!indexes.is_empty(), "should have at least the _id index");
    assert!(indexes.iter().any(|idx| idx.primary), "should have a primary index on _id");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_invalid_query() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();

    let r = provider.execute_query("not valid json").await;
    assert!(r.is_err(), "invalid JSON should error");

    let r = provider.execute_query(r#"{"unknown_cmd": "x"}"#).await;
    assert!(r.is_err(), "unknown command should error");

    let r = provider.execute_query(r#"[]"#).await;
    assert!(r.is_err(), "array query should error");
}

#[tokio::test]
async fn test_mongodb_explorer_flow() {
    let config = get_config().unwrap();
    let connected_db = config.database.clone();
    let mut provider = MongoProvider::new();
    provider.connect(&config).await.unwrap();

    // Insert a test doc so the collection exists
    let coll = coll_name("explorer");
    let insert = format!(r#"{{"insertOne": "{}", "document": {{"x": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    // ── Simulate connect_to loop ──
    let databases = provider.get_databases().await.unwrap();
    assert!(!databases.is_empty(), "should list at least one database");
    assert!(
        databases.contains(&connected_db),
        "connected db '{}' should be in: {:?}",
        connected_db, databases
    );

    let mut schemas_data: Vec<(String, Vec<String>)> = Vec::new();
    let mut tables_data: Vec<(String, Vec<dbrusty::database::types::TableInfo>)> = Vec::new();

    for db in &databases {
        let mut schemas = provider.get_schemas().await.unwrap_or_default();
        // No explicit schemas (e.g. MongoDB) → use db name as synthetic schema
        if schemas.is_empty() {
            schemas = vec![db.clone()];
        }
        schemas_data.push((db.clone(), schemas.clone()));

        for schema in &schemas {
            let schema_key = format!("{}.{}", db, schema);
            if let Ok(tables) = provider.get_tables(schema).await {
                tables_data.push((schema_key.clone(), tables));
            }
        }
    }

    // ── Check the connected db entry ──
    let db_schemas = schemas_data
        .iter()
        .find(|(s, _)| s == &connected_db)
        .expect("connected db should have schema data");
    assert_eq!(db_schemas.1.len(), 1, "connected db should have 1 synthetic schema");
    assert_eq!(db_schemas.1[0], connected_db, "synthetic schema should match db name");

    // The flat layout key is "{db}.{db}"
    let flat_key = format!("{}.{}", connected_db, connected_db);
    let db_tables = tables_data.iter().find(|(key, _)| key == &flat_key);
    assert!(
        db_tables.is_some(),
        "tables_data should have entry for key '{}'.\n  schemas_data: {:?}\n  tables_data keys: {:?}",
        flat_key,
        schemas_data,
        tables_data.iter().map(|(k, _)| k).collect::<Vec<_>>(),
    );

    let (_, tables) = db_tables.unwrap();
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&coll.as_str()),
        "explorer collection '{}' should be in tables: {:?}",
        coll, names
    );

    // Cleanup
    clear_collection(&provider, &coll).await;
}
