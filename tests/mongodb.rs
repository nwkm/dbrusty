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

    let q = format!(
        r#"{{"insertOne": "{}", "document": {{"name": "alice", "age": 30}}}}"#,
        coll
    );
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

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"v": 1}}, {{"v": 2}}]}}"#,
        coll
    );
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

    let insert = format!(
        r#"{{"insertOne": "{}", "document": {{"item": "x", "status": "old"}}}}"#,
        coll
    );
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

    let q = format!(
        r#"{{"count": "{}", "filter": {{"a": {{"$gt": 1}}}}}}"#,
        coll
    );
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

    let q = format!(
        r#"{{"distinct": "{}", "field": "color", "filter": {{}}}}"#,
        coll
    );
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
    assert!(
        names.iter().any(|n| *n == coll),
        "collection should appear in get_tables"
    );

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
    assert!(
        indexes.iter().any(|idx| idx.primary),
        "should have a primary index on _id"
    );

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
        connected_db,
        databases
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
    assert_eq!(
        db_schemas.1.len(),
        1,
        "connected db should have 1 synthetic schema"
    );
    assert_eq!(
        db_schemas.1[0], connected_db,
        "synthetic schema should match db name"
    );

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
        coll,
        names
    );

    // Cleanup
    clear_collection(&provider, &coll).await;
}

// ── CRUD edge cases ────────────────────────────────────────────────

#[tokio::test]
async fn test_mongodb_insert_all_bson_types() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("bson_types");

    let q = format!(
        r#"{{
            "insertOne": "{}",
            "document": {{
                "str": "hello",
                "int": 42,
                "double": 3.14,
                "bool": true,
                "null": null,
                "arr": [1, 2, 3],
                "nested": {{"a": 1, "b": "x"}}
            }}
        }}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 1);

    let find = format!(r#"{{"find": "{}"}}"#, coll);
    let r = provider.execute_query(&find).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_insert_empty_document() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("empty_doc");

    let q = format!(r#"{{"insertOne": "{}", "document": {{}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.rows_affected, 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_insert_many_empty_array() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("insert_many_empty");

    let q = format!(r#"{{"insertMany": "{}", "documents": []}}"#, coll);
    let r = provider.execute_query(&q).await;
    assert!(r.is_err(), "insertMany with empty array should error");
}

#[tokio::test]
async fn test_mongodb_find_with_sort() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("find_sort");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"order": 3}}, {{"order": 1}}, {{"order": 2}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    // Ascending sort
    let q = format!(r#"{{"find": "{}", "sort": {{"order": 1}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 3);
    let docs = r.raw_documents.as_ref().unwrap();
    let order0 = docs[0].get("order").and_then(|v| v.as_i32()).unwrap();
    assert_eq!(order0, 1, "asc sort first should have order=1");

    // Descending sort
    let q = format!(r#"{{"find": "{}", "sort": {{"order": -1}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    let docs = r.raw_documents.as_ref().unwrap();
    let order0 = docs[0].get("order").and_then(|v| v.as_i32()).unwrap();
    assert_eq!(order0, 3, "desc sort first should have order=3");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_find_with_logical_operators() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("logical_ops");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"x": 1, "y": "a"}}, {{"x": 2, "y": "b"}}, {{"x": 1, "y": "b"}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    // $and
    let q = format!(
        r#"{{"find": "{}", "filter": {{"$and": [{{"x": 1}}, {{"y": "a"}}]}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1, "$and should match 1 doc");

    // $or
    let q = format!(
        r#"{{"find": "{}", "filter": {{"$or": [{{"x": 2}}, {{"y": "a"}}]}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 2, "$or should match 2 docs");

    // $nor
    let q = format!(
        r#"{{"find": "{}", "filter": {{"$nor": [{{"x": 1}}, {{"y": "a"}}]}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1, "$nor should match 1 doc");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_find_with_elem_match() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("elem_match");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"items": [{{"qty": 5, "sku": "A"}}, {{"qty": 2, "sku": "B"}}]}}, {{"items": [{{"qty": 1, "sku": "A"}}]}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"find": "{}", "filter": {{"items": {{"$elemMatch": {{"qty": {{"$gt": 3}}, "sku": "A"}}}}}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_find_with_nested_field() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("nested_field");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"address": {{"city": "NYC", "zip": "10001"}}}}, {{"address": {{"city": "LA", "zip": "90001"}}}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"find": "{}", "filter": {{"address.city": "NYC"}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_update_with_operators() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("update_ops");

    let insert = format!(
        r#"{{"insertOne": "{}", "document": {{"count": 10, "tags": ["a"], "name": "old", "active": true}}}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let find = format!(r#"{{"find": "{}"}}"#, coll);

    // Helper: extract a field from the first raw document
    async fn get_field_val(p: &MongoProvider, q: &str, field: &str) -> bson::Bson {
        let r2 = p.execute_query(q).await.unwrap();
        let d2 = r2.raw_documents.as_ref().unwrap();
        d2[0].get(field).cloned().unwrap_or(bson::Bson::Null)
    }

    // $inc
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{}}, "update": {{"$inc": {{"count": 5}}}}}}"#,
        coll
    );
    provider.execute_query(&q).await.unwrap();
    let count = get_field_val(&provider, &find, "count").await;
    assert_eq!(count.as_i32(), Some(15), "$inc failed");

    // $push to array
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{}}, "update": {{"$push": {{"tags": "b"}}}}}}"#,
        coll
    );
    provider.execute_query(&q).await.unwrap();
    let tags = get_field_val(&provider, &find, "tags").await;
    assert!(tags.as_array().is_some(), "$push should produce array");
    let arr = tags.as_array().unwrap();
    assert!(arr.iter().any(|v| v.as_str() == Some("b")), "$push should add 'b'");

    // $unset
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{}}, "update": {{"$unset": {{"active": ""}}}}}}"#,
        coll
    );
    provider.execute_query(&q).await.unwrap();
    let r = provider.execute_query(&find).await.unwrap();
    let docs = r.raw_documents.as_ref().unwrap();
    assert!(docs[0].get("active").is_none(), "$unset should remove active");

    // $rename
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{}}, "update": {{"$rename": {{"name": "title"}}}}}}"#,
        coll
    );
    provider.execute_query(&q).await.unwrap();
    let r = provider.execute_query(&find).await.unwrap();
    let docs = r.raw_documents.as_ref().unwrap();
    assert!(docs[0].get("title").is_some(), "$rename should create title");
    assert!(docs[0].get("name").is_none(), "$rename should remove name");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_update_upsert() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("upsert");

    // Upsert inserts a new doc if no match
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{"sku": "X"}}, "update": {{"$set": {{"qty": 100}}}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    // Default (no upsert flag) should affect 0
    assert_eq!(r.rows_affected, 0, "without upsert should affect 0");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_update_no_match() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("update_no_match");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"k": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    // Update with filter that matches nothing
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{"k": 999}}, "update": {{"$set": {{"k": 0}}}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.rows_affected, 0);

    // Verify original doc unchanged
    let r = provider.execute_query(&format!(r#"{{"find": "{}"}}"#, coll)).await.unwrap();
    let docs = r.raw_documents.as_ref().unwrap();
    let k = docs[0].get("k").and_then(|v| v.as_i32()).unwrap();
    assert_eq!(k, 1, "document should be unchanged");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_delete_no_match() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("delete_no_match");

    let insert = format!(r#"{{"insertOne": "{}", "document": {{"v": 1}}}}"#, coll);
    provider.execute_query(&insert).await.unwrap();

    // DeleteOne with filter matching nothing
    let q = format!(r#"{{"deleteOne": "{}", "filter": {{"v": 999}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.rows_affected, 0);

    // DeleteMany with filter matching nothing
    let q = format!(r#"{{"deleteMany": "{}", "filter": {{"v": 999}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.rows_affected, 0);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_replace_one() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("replace");

    let insert = format!(
        r#"{{"insertOne": "{}", "document": {{"name": "original", "val": 1}}}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    // Use updateOne with $set to simulate replace
    let q = format!(
        r#"{{"updateOne": "{}", "filter": {{}}, "update": {{"$set": {{"name": "replaced", "val": 2}}}}}}"#,
        coll
    );
    let _r = provider.execute_query(&q).await.unwrap();
    let r2 = provider.execute_query(&format!(r#"{{"find": "{}"}}"#, coll)).await.unwrap();
    let docs = r2.raw_documents.as_ref().unwrap();
    let name = docs[0].get("name").and_then(|v| v.as_str()).unwrap();
    assert_eq!(name, "replaced", "update not applied");

    clear_collection(&provider, &coll).await;
}

// ── Aggregation tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_mongodb_aggregate_match_sort_limit() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_match_sort");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"v": 3}}, {{"v": 1}}, {{"v": 2}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    // $match + $sort + $limit
    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$match": {{"v": {{"$gte": 2}}}}}}, {{"$sort": {{"v": 1}}}}, {{"$limit": 1}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
    let docs = r.raw_documents.as_ref().unwrap();
    let v = docs[0].get("v").and_then(|v| v.as_i32()).unwrap();
    assert_eq!(v, 2, "expected v=2 after sort+limit");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_group_sum() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_group_sum");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"cat": "A", "val": 10}}, {{"cat": "A", "val": 20}}, {{"cat": "B", "val": 5}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$group": {{"_id": "$cat", "total": {{"$sum": "$val"}}, "count": {{"$sum": 1}}}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 2);

    for row in &r.rows {
        let s = row[0].to_string();
        if s.contains("\"A\"") {
            assert!(s.contains("\"total\": 30"), "A total should be 30: {}", s);
            assert!(s.contains("\"count\": 2"), "A count should be 2: {}", s);
        }
    }

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_project() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_project");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"a": 1, "b": 2, "c": 3}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$project": {{"a": 1, "b": 1, "_id": 0}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);
    let docs = r.raw_documents.as_ref().unwrap();
    assert!(docs[0].get("a").is_some(), "project should include a");
    assert!(docs[0].get("b").is_some(), "project should include b");
    assert!(docs[0].get("c").is_none(), "project should exclude c");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_unwind() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_unwind");

    let insert = format!(
        r#"{{"insertOne": "{}", "document": {{"items": ["x", "y", "z"]}}}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$unwind": "$items"}}, {{"$project": {{"item": "$items", "_id": 0}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 3);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_add_fields() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_add_fields");

    let insert = format!(
        r#"{{"insertOne": "{}", "document": {{"price": 100, "tax_rate": 0.08}}}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$addFields": {{"total": {{"$add": ["$price", {{"$multiply": ["$price", "$tax_rate"]}}]}}}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);
    let docs = r.raw_documents.as_ref().unwrap();
    assert!(docs[0].get("total").is_some(), "$addFields should add total");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_group_accumulators() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_accumulators");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"g": 1, "v": 10}}, {{"g": 1, "v": 20}}, {{"g": 1, "v": 30}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$group": {{"_id": "$g", "minVal": {{"$min": "$v"}}, "maxVal": {{"$max": "$v"}}, "avgVal": {{"$avg": "$v"}}, "count": {{"$sum": 1}}}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);
    let docs = r.raw_documents.as_ref().unwrap();
    let min_val = docs[0].get("minVal").and_then(|v| v.as_i32()).unwrap();
    let max_val = docs[0].get("maxVal").and_then(|v| v.as_i32()).unwrap();
    let avg_val = docs[0].get("avgVal").and_then(|v| v.as_f64()).unwrap();
    assert_eq!(min_val, 10, "min should be 10");
    assert_eq!(max_val, 30, "max should be 30");
    assert_eq!(avg_val, 20.0, "avg should be 20");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_bucket() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_bucket");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"score": 15}}, {{"score": 25}}, {{"score": 35}}, {{"score": 45}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$bucket": {{"groupBy": "$score", "boundaries": [0, 20, 40, 60], "default": "Other", "output": {{"count": {{"$sum": 1}}, "scores": {{"$push": "$score"}}}}}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 3);

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_aggregate_facet() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("agg_facet");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"type": "A", "price": 10}}, {{"type": "A", "price": 20}}, {{"type": "B", "price": 30}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"aggregate": "{}", "pipeline": [{{"$facet": {{"byType": [{{"$group": {{"_id": "$type", "count": {{"$sum": 1}}}}}}], "avgPrice": [{{"$group": {{"_id": null, "avg": {{"$avg": "$price"}}}}}}]}}}}]}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);

    clear_collection(&provider, &coll).await;
}

// ── Count edge cases ───────────────────────────────────────────────

#[tokio::test]
async fn test_mongodb_count_empty_filter() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("count_empty");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"x": 1}}, {{"x": 2}}, {{"x": 3}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"count": "{}", "filter": {{}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
    if let Some(Value::Integer(n)) = r.rows.get(0).and_then(|row| row.get(0)) {
        assert_eq!(*n, 3, "count all should be 3");
    }

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_count_no_match() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("count_no_match");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"x": 1}}, {{"x": 2}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(r#"{{"count": "{}", "filter": {{"x": 999}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 1);
    if let Some(Value::Integer(n)) = r.rows.get(0).and_then(|row| row.get(0)) {
        assert_eq!(*n, 0, "count no match should be 0");
    }

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_count_with_regex() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("count_regex");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"name": "apple"}}, {{"name": "banana"}}, {{"name": "avocado"}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"count": "{}", "filter": {{"name": {{"$regex": "^a"}}}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    if let Some(Value::Integer(n)) = r.rows.get(0).and_then(|row| row.get(0)) {
        assert_eq!(*n, 2, "count with regex ^a should be 2");
    }

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_count_on_empty_collection() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("count_empty_coll");

    let q = format!(r#"{{"count": "{}", "filter": {{}}}}"#, coll);
    let r = provider.execute_query(&q).await.unwrap();
    if let Some(Value::Integer(n)) = r.rows.get(0).and_then(|row| row.get(0)) {
        assert_eq!(*n, 0, "count on empty collection should be 0");
    }
}

// ── Distinct edge cases ────────────────────────────────────────────

#[tokio::test]
async fn test_mongodb_distinct_with_filter() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("distinct_filter");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"status": "active", "val": 1}}, {{"status": "active", "val": 2}}, {{"status": "inactive", "val": 1}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"distinct": "{}", "field": "val", "filter": {{"status": "active"}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 2, "should have 2 distinct vals for active");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_distinct_nested_field() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("distinct_nested");

    let insert = format!(
        r#"{{"insertMany": "{}", "documents": [{{"addr": {{"city": "NYC"}}}}, {{"addr": {{"city": "LA"}}}}, {{"addr": {{"city": "NYC"}}}}]}}"#,
        coll
    );
    provider.execute_query(&insert).await.unwrap();

    let q = format!(
        r#"{{"distinct": "{}", "field": "addr.city", "filter": {{}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert_eq!(r.row_count(), 2, "should have 2 distinct cities");

    clear_collection(&provider, &coll).await;
}

#[tokio::test]
async fn test_mongodb_distinct_on_empty_collection() {
    let mut provider = MongoProvider::new();
    provider.connect(&get_config().unwrap()).await.unwrap();
    let coll = coll_name("distinct_empty");

    let q = format!(
        r#"{{"distinct": "{}", "field": "x", "filter": {{}}}}"#,
        coll
    );
    let r = provider.execute_query(&q).await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 0, "distinct on empty collection should be 0");
}
