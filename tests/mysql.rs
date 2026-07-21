/// Integration test for the MySQL provider.
///
/// Set these environment variables to run:
///   DATABASE_HOST (default: 127.0.0.1)
///   DATABASE_PORT (default: 3306)
///   DATABASE_NAME (required)
///   DATABASE_USERNAME (required)
///   DATABASE_PASSWORD (required)
///
/// Test is skipped if DATABASE_USERNAME is not set.
use dbrusty::database::provider::DatabaseProvider;
use dbrusty::models::connection::{ConnectionConfig, DatabaseType};
use dbrusty::providers::mysql::MysqlProvider;

fn get_config() -> Option<ConnectionConfig> {
    let username = std::env::var("DATABASE_USERNAME").ok()?;
    if username.is_empty() || username == "<change me>" {
        return None;
    }
    let password = std::env::var("DATABASE_PASSWORD").ok().unwrap_or_default();
    if password.is_empty() || password == "<change me>" {
        return None;
    }

    let host = std::env::var("DATABASE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("DATABASE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3306);
    let database = std::env::var("DATABASE_NAME").ok()?;
    if database.is_empty() || database == "<change me>" {
        return None;
    }

    Some(ConnectionConfig {
        id: uuid::Uuid::new_v4(),
        name: "test-mysql".into(),
        db_type: DatabaseType::MySQL,
        host,
        port,
        database,
        username,
        group: None,
        is_favorite: false,
        ssl: false,
        created_at: chrono::Utc::now(),
        last_connected: None,
        password: Some(password),
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

async fn ensure_provider(config: &ConnectionConfig) -> MysqlProvider {
    let mut provider = MysqlProvider::new();
    provider.connect(config).await.unwrap();
    provider
}

#[tokio::test]
async fn test_mysql_connect_and_ping() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let mut provider = ensure_provider(&config).await;
    assert!(provider.ping().await.unwrap());
    let info = provider.server_info().await.unwrap();
    assert!(!info.version.is_empty());
    assert_eq!(info.vendor, "MySQL");
    provider.disconnect().await.unwrap();
}

#[tokio::test]
async fn test_mysql_get_databases() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let databases = provider.get_databases().await.unwrap();
    assert!(databases.contains(&config.database));
}

#[tokio::test]
async fn test_mysql_get_tables() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("get_tables");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))",
            tbl
        ))
        .await
        .unwrap();
    let tables = provider.get_tables(&config.database).await.unwrap();
    assert!(tables.iter().any(|t| t.name == tbl));
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mysql_get_columns() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = format!("{}.test_columns", config.database);
    provider
        .execute_query(&format!(
            "CREATE TABLE test_columns (id INT AUTO_INCREMENT PRIMARY KEY, label VARCHAR(100) NOT NULL, active TINYINT(1) DEFAULT 1, price DECIMAL(10,2))"
        ))
        .await
        .unwrap();
    let columns = provider.get_columns(&tbl).await.unwrap();
    assert!(columns.iter().any(|c| c.name == "id" && c.is_pk && c.is_auto_increment));
    assert!(columns.iter().any(|c| c.name == "label" && !c.nullable));
    assert!(columns.iter().any(|c| c.name == "active"));
    assert!(columns.iter().any(|c| c.name == "price"));
    provider
        .execute_query("DROP TABLE test_columns")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mysql_get_indexes() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("indexes");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, email VARCHAR(100) UNIQUE, name VARCHAR(50))",
            tbl
        ))
        .await
        .unwrap();
    let indexes = provider
        .get_indexes(&format!("{}.{}", config.database, tbl))
        .await
        .unwrap();
    assert!(indexes.iter().any(|i| i.primary));
    if let Some(email_idx) = indexes.iter().find(|i| i.columns.contains(&"email".to_string())) {
        assert!(email_idx.unique);
    }
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mysql_create_table_all_types() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("all_types");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (\
             id INT AUTO_INCREMENT PRIMARY KEY, \
             tiny TINYINT, \
             sml SMALLINT, \
             med MEDIUMINT, \
             big BIGINT, \
             dec_col DECIMAL(10,2), \
             fl FLOAT, \
             dbl DOUBLE, \
             txt TEXT, \
             vc VARCHAR(100), \
             is_active TINYINT(1), \
             dt DATE, \
             ts TIMESTAMP, \
             j JSON,
             bl BLOB
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
async fn test_mysql_insert_select() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("insert_select");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50), score INT)",
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
async fn test_mysql_where_operators() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("where_ops");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50), score INT, active TINYINT(1))",
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
        .execute_query(&format!(
            "SELECT * FROM {} WHERE name LIKE 'A%'",
            tbl
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
async fn test_mysql_aggregates() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("agg");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, cat VARCHAR(20), val INT)",
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
async fn test_mysql_order_limit() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("order_limit");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)",
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
async fn test_mysql_join() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let t1 = table_name("t1");
    let t2 = table_name("t2");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))",
            t1
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, t1_id INT, label VARCHAR(50))",
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
async fn test_mysql_subquery() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("subq");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, dept VARCHAR(20), val INT)",
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
async fn test_mysql_update() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("update");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50), score INT)",
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
        .execute_query(&format!(
            "SELECT score FROM {} WHERE name = 'Alice'",
            tbl
        ))
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
async fn test_mysql_delete() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("delete");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name) VALUES ('Alice')",
            tbl
        ))
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
async fn test_mysql_transaction_commit() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_commit");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)",
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
async fn test_mysql_transaction_rollback() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_rollback");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)",
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
async fn test_mysql_null_handling() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("nulls");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50), score INT)",
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
        .execute_query(&format!(
            "SELECT * FROM {} WHERE score IS NULL",
            tbl
        ))
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
async fn test_mysql_distinct() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("distinct");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, city VARCHAR(50))",
            tbl
        ))
        .await
        .unwrap();
    for city in &["NYC", "LA", "NYC", "SF", "LA"] {
        provider
            .execute_query(&format!(
                "INSERT INTO {} (city) VALUES ('{}')",
                tbl, city
            ))
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
async fn test_mysql_union() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let t1 = table_name("u1");
    let t2 = table_name("u2");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT, label VARCHAR(20))",
            t1
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT, label VARCHAR(20))",
            t2
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} VALUES (1, 'A')", t1))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} VALUES (2, 'B')", t2))
        .await
        .unwrap();
    let result = provider
        .execute_query(&format!(
            "SELECT label FROM {} UNION SELECT label FROM {} ORDER BY label",
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
async fn test_mysql_get_views() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("view_base");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))",
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
    let views = provider.get_views(&config.database).await.unwrap();
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
async fn test_mysql_get_functions() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let fn_name = table_name("fn");
    provider
        .execute_query(&format!(
            "CREATE FUNCTION {}() RETURNS INT DETERMINISTIC RETURN 42",
            fn_name
        ))
        .await
        .unwrap();
    let functions = provider.get_functions(&config.database).await.unwrap();
    assert!(functions.iter().any(|f| f.name == fn_name));
    provider
        .execute_query(&format!("DROP FUNCTION {}", fn_name))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mysql_get_procedures() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let proc_name = table_name("proc");
    provider
        .execute_query(&format!(
            "CREATE PROCEDURE {}() BEGIN SELECT 1; END",
            proc_name
        ))
        .await
        .unwrap();
    let procedures = provider.get_procedures(&config.database).await.unwrap();
    assert!(procedures.iter().any(|p| p.name == proc_name));
    provider
        .execute_query(&format!("DROP PROCEDURE {}", proc_name))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mysql_get_triggers() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("trig_base");
    let trig_name = table_name("trig");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, val INT, val_log INT DEFAULT 0)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE TRIGGER {} BEFORE INSERT ON {} FOR EACH ROW SET NEW.val_log = NEW.val * 2",
            trig_name, tbl
        ))
        .await
        .unwrap();
    let triggers = provider.get_triggers(&config.database).await.unwrap();
    assert!(triggers.iter().any(|t| t.name == trig_name));
    provider
        .execute_query(&format!("DROP TRIGGER {}", trig_name))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_mysql_explain() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("explain");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))",
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
async fn test_mysql_large_result() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("large");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)",
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
async fn test_mysql_empty_result() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("empty");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))",
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

#[tokio::test]
async fn test_mysql_show_databases() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let result = provider
        .execute_query("SHOW DATABASES")
        .await
        .unwrap();
    assert!(result.is_select);
    assert!(!result.rows.is_empty());
}
