/// Integration test for the SQLite provider.
///
/// SQLite is embedded (no external process needed). The default database
/// is in-memory (`sqlite://:memory:`), so these tests always run unless
/// overridden by setting DATABASE_PATH.

use dbrusty::database::provider::DatabaseProvider;
use dbrusty::models::connection::{ConnectionConfig, DatabaseType};
use dbrusty::providers::sqlite::SqliteProvider;

fn get_config() -> Option<ConnectionConfig> {
    let database = std::env::var("DATABASE_PATH").unwrap_or_else(|_| ":memory:".into());

    Some(ConnectionConfig {
        id: uuid::Uuid::new_v4(),
        name: "test-sqlite".into(),
        db_type: DatabaseType::SQLite,
        host: String::new(),
        port: 0,
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

fn table_name(prefix: &str) -> String {
    let suffix: String = uuid::Uuid::new_v4().to_string()[..8].to_string();
    format!("test_{}_{}", prefix, suffix)
}

fn skip_if_no_config() -> Option<ConnectionConfig> {
    let config = get_config()?;
    Some(config)
}

async fn ensure_provider(config: &ConnectionConfig) -> SqliteProvider {
    let mut provider = SqliteProvider::new();
    provider.connect(config).await.unwrap();
    provider
}

#[tokio::test]
async fn test_sqlite_connect_and_ping() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let mut provider = ensure_provider(&config).await;
    assert!(provider.ping().await.unwrap());
    let info = provider.server_info().await.unwrap();
    assert!(!info.version.is_empty());
    assert_eq!(info.vendor, "SQLite");
    provider.disconnect().await.unwrap();
}

#[tokio::test]
async fn test_sqlite_get_databases() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let databases = provider.get_databases().await.unwrap();
    assert_eq!(databases, vec!["main"]);
}

#[tokio::test]
async fn test_sqlite_get_schemas() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let schemas = provider.get_schemas().await.unwrap();
    assert_eq!(schemas, vec!["main"]);
}

#[tokio::test]
async fn test_sqlite_get_tables() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("get_tables");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    let tables = provider.get_tables("main").await.unwrap();
    assert!(tables.iter().any(|t| t.name == tbl));
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_get_columns() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("columns");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT NOT NULL, active INTEGER DEFAULT 1, price REAL)",
            tbl
        ))
        .await
        .unwrap();
    let columns = provider.get_columns(&format!("main.{}", tbl)).await.unwrap();
    assert!(columns.iter().any(|c| c.name == "id" && c.is_pk));
    assert!(columns.iter().any(|c| c.name == "label" && !c.nullable));
    assert!(columns.iter().any(|c| c.name == "active"));
    assert!(columns.iter().any(|c| c.name == "price"));
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_get_indexes() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("indexes");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT UNIQUE, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    let indexes = provider
        .get_indexes(&format!("main.{}", tbl))
        .await
        .unwrap();
    if let Some(email_idx) = indexes
        .iter()
        .find(|i| i.columns.contains(&"email".to_string()))
    {
        assert!(email_idx.unique);
    }
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_get_views() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("view_base");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    let view_name = format!("v_{}", tbl);
    provider
        .execute_query(&format!(
            "CREATE VIEW {} AS SELECT * FROM {}",
            view_name, tbl
        ))
        .await
        .unwrap();
    let views = provider.get_views("main").await.unwrap();
    assert!(views.iter().any(|v| v.name == view_name));
    provider
        .execute_query(&format!("DROP VIEW {}", view_name))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_get_procedures() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let procedures = provider.get_procedures("main").await.unwrap();
    assert!(procedures.is_empty());
}

#[tokio::test]
async fn test_sqlite_create_table_all_types() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("all_types");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             small INTEGER, \
             big INTEGER, \
             real_col REAL, \
             txt TEXT, \
             blb BLOB, \
             flag INTEGER DEFAULT 0\
             )",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_insert_select() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("insert_select");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, score) VALUES ('Alice', 100)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, score) VALUES ('Bob', 200)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, score) VALUES ('Charlie', 300)",
            tbl
        ))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!("SELECT * FROM {} ORDER BY id", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.columns.len(), 3);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_where_operators() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("where_ops");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score INTEGER, active INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    for (name, score, active) in &[("Alice", 100, 1), ("Bob", 200, 1), ("Charlie", 300, 0)] {
        provider
            .execute_query(&format!(
                "INSERT INTO {} (name, score, active) VALUES ('{}', {}, {})",
                tbl, name, score, active
            ))
            .await
            .unwrap();
    }
    let result = provider
        .execute_query(&format!("SELECT * FROM {} WHERE score > 150", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    let result = provider
        .execute_query(&format!("SELECT * FROM {} WHERE active = 1", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    let result = provider
        .execute_query(&format!(
            "SELECT * FROM {} WHERE name IN ('Alice', 'Charlie')",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    let result = provider
        .execute_query(&format!(
            "SELECT * FROM {} WHERE score BETWEEN 50 AND 250",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    let result = provider
        .execute_query(&format!("SELECT * FROM {} WHERE name LIKE 'A%'", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_aggregates() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("agg");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, cat TEXT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    for (cat, val) in &[("A", 10), ("A", 20), ("B", 30), ("B", 40)] {
        provider
            .execute_query(&format!(
                "INSERT INTO {} (cat, val) VALUES ('{}', {})",
                tbl, cat, val
            ))
            .await
            .unwrap();
    }
    let result = provider
        .execute_query(&format!("SELECT COUNT(*) AS cnt FROM {}", tbl))
        .await
        .unwrap();
    let cnt: i64 = result.rows[0][0].to_string().parse().unwrap();
    assert_eq!(cnt, 4);
    let result = provider
        .execute_query(&format!("SELECT SUM(val) AS s FROM {}", tbl))
        .await
        .unwrap();
    let sum: i64 = result.rows[0][0].to_string().parse().unwrap();
    assert_eq!(sum, 100);
    let result = provider
        .execute_query(&format!(
            "SELECT cat, COUNT(*) AS cnt FROM {} GROUP BY cat ORDER BY cat",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_order_limit() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("order_limit");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    for i in 1..=5 {
        provider
            .execute_query(&format!("INSERT INTO {} (val) VALUES ({})", tbl, i * 10))
            .await
            .unwrap();
    }
    let result = provider
        .execute_query(&format!(
            "SELECT val FROM {} ORDER BY val DESC LIMIT 2",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_join() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let t1 = table_name("t1");
    let t2 = table_name("t2");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            t1
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, t1_id INTEGER, label TEXT)",
            t2
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (name) VALUES ('Alice')", t1))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (name) VALUES ('Bob')", t1))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (t1_id, label) VALUES (1, 'A')",
            t2
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (t1_id, label) VALUES (2, 'B')",
            t2
        ))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!(
            "SELECT t1.name, t2.label FROM {} t1 JOIN {} t2 ON t1.id = t2.t1_id ORDER BY t1.id",
            t1, t2
        ))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 2);
    provider
        .execute_query(&format!("DROP TABLE {}", t2))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", t1))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_subquery() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("subq");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, dept TEXT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    for (dept, val) in &[("Eng", 100), ("Eng", 200), ("Sales", 300)] {
        provider
            .execute_query(&format!(
                "INSERT INTO {} (dept, val) VALUES ('{}', {})",
                tbl, dept, val
            ))
            .await
            .unwrap();
    }
    let result = provider
        .execute_query(&format!(
            "SELECT * FROM {} WHERE val > (SELECT AVG(val) FROM {})",
            tbl, tbl
        ))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_update() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("update");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, score) VALUES ('Alice', 100)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, score) VALUES ('Bob', 200)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "UPDATE {} SET score = 150 WHERE name = 'Alice'",
            tbl
        ))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!("SELECT score FROM {} WHERE name = 'Alice'", tbl))
        .await
        .unwrap();
    let score: i64 = result.rows[0][0].to_string().parse().unwrap();
    assert_eq!(score, 150);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_delete() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("delete");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (name) VALUES ('Alice')", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (name) VALUES ('Bob')", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DELETE FROM {} WHERE name = 'Alice'", tbl))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!("SELECT COUNT(*) AS cnt FROM {}", tbl))
        .await
        .unwrap();
    let cnt: i64 = result.rows[0][0].to_string().parse().unwrap();
    assert_eq!(cnt, 1);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_transaction_commit() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_commit");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    provider.begin_transaction().await.unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES (42)", tbl))
        .await
        .unwrap();
    provider.commit().await.unwrap();
    let result = provider
        .execute_query(&format!("SELECT val FROM {} WHERE id = 1", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_transaction_rollback() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_rollback");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    provider.begin_transaction().await.unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES (99)", tbl))
        .await
        .unwrap();
    provider.rollback().await.unwrap();
    let result = provider
        .execute_query(&format!("SELECT COUNT(*) AS cnt FROM {}", tbl))
        .await
        .unwrap();
    let cnt: i64 = result.rows[0][0].to_string().parse().unwrap();
    assert_eq!(cnt, 0);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_null_handling() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("nulls");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, score) VALUES ('HasNull', NULL)",
            tbl
        ))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!("SELECT * FROM {} WHERE score IS NULL", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    let result = provider
        .execute_query(&format!("SELECT * FROM {} WHERE score IS NOT NULL", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_distinct() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("distinct");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, city TEXT)",
            tbl
        ))
        .await
        .unwrap();
    for city in &["NYC", "LA", "NYC", "SF", "LA"] {
        provider
            .execute_query(&format!("INSERT INTO {} (city) VALUES ('{}')", tbl, city))
            .await
            .unwrap();
    }
    let result = provider
        .execute_query(&format!("SELECT DISTINCT city FROM {} ORDER BY city", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 3);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_explain() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("explain");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!("EXPLAIN SELECT * FROM {}", tbl))
        .await
        .unwrap();
    assert!(result.is_select);
    assert!(!result.rows.is_empty());
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_large_result() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("large");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    let mut sql = format!("INSERT INTO {} (val) VALUES ", tbl);
    let values: Vec<String> = (1..=100).map(|i| format!("({})", i)).collect();
    sql.push_str(&values.join(", "));
    provider.execute_query(&sql).await.unwrap();
    let result = provider
        .execute_query(&format!("SELECT COUNT(*) AS cnt FROM {}", tbl))
        .await
        .unwrap();
    let cnt: i64 = result.rows[0][0].to_string().parse().unwrap();
    assert_eq!(cnt, 100);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_sqlite_empty_result() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("empty");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!("SELECT * FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}
