/// Integration test for the PostgreSQL provider.
///
/// Set these environment variables to run:
///   DATABASE_HOST (default: 127.0.0.1)
///   DATABASE_PORT (default: 5432)
///   DATABASE_NAME (default: strapi)
///   DATABASE_USERNAME (required)
///   DATABASE_PASSWORD (required)
///
/// Test is skipped if DATABASE_USERNAME is not set.
use dbrusty::database::provider::DatabaseProvider;
use dbrusty::models::connection::{ConnectionConfig, DatabaseType};
use dbrusty::providers::postgres::PostgresProvider;

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
        .unwrap_or(5432);
    let database = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "strapi".into());

    Some(ConnectionConfig {
        id: uuid::Uuid::new_v4(),
        name: "test-postgres".into(),
        db_type: DatabaseType::Postgres,
        host,
        port,
        database,
        username,
        group: None,
        is_favorite: false,
        ssl: false,
        created_at: chrono::Utc::now(),
        last_connected: None,
        password: None,
    })
}

#[tokio::test]
async fn test_postgres_connect_and_ping() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    let result = provider.connect(&config).await;
    assert!(result.is_ok(), "connect failed: {:?}", result.err());

    let ping = provider.ping().await;
    assert!(ping.is_ok(), "ping failed: {:?}", ping.err());
    assert!(ping.unwrap(), "ping returned false");
}

#[tokio::test]
async fn test_postgres_get_databases() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let databases = provider
        .get_databases()
        .await
        .expect("get_databases failed");
    assert!(!databases.is_empty(), "should have at least one database");
    assert!(
        databases.contains(&config.database),
        "expected '{}' in databases list",
        config.database
    );
}

#[tokio::test]
async fn test_postgres_get_schemas() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let schemas = provider.get_schemas().await.expect("get_schemas failed");
    assert!(!schemas.is_empty(), "should have at least 'public' schema");
    assert!(
        schemas.contains(&"public".to_string()),
        "expected 'public' schema"
    );
}

#[tokio::test]
async fn test_postgres_get_tables() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let tables = provider
        .get_tables("public")
        .await
        .expect("get_tables failed");
    // We don't assert on count — the database may be empty
    for t in &tables {
        assert!(!t.name.is_empty(), "table name should not be empty");
        assert_eq!(t.schema, "public");
    }
}

#[tokio::test]
async fn test_postgres_execute_query() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let result = provider
        .execute_query("SELECT 1 AS num, 'hello' AS text, true AS flag")
        .await;
    assert!(result.is_ok(), "query failed: {:?}", result.err());

    let result = result.unwrap();
    assert!(result.is_select, "should be a select query");
    assert_eq!(result.col_count(), 3, "should have 3 columns");
    assert_eq!(result.row_count(), 1, "should have 1 row");
}

#[tokio::test]
async fn test_postgres_server_info() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let info = provider.server_info().await.expect("server_info failed");
    assert_eq!(info.vendor, "PostgreSQL");
    assert!(!info.version.is_empty(), "version should not be empty");
}

#[tokio::test]
async fn test_postgres_transactions() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let result = provider.begin_transaction().await;
    assert!(
        result.is_ok(),
        "begin_transaction failed: {:?}",
        result.err()
    );

    let result = provider.rollback().await;
    assert!(result.is_ok(), "rollback failed: {:?}", result.err());

    let result = provider.begin_transaction().await;
    assert!(
        result.is_ok(),
        "begin_transaction failed: {:?}",
        result.err()
    );

    let result = provider.commit().await;
    assert!(result.is_ok(), "commit failed: {:?}", result.err());
}

#[tokio::test]
async fn test_postgres_get_columns() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let tables = provider
        .get_tables("public")
        .await
        .expect("get_tables failed");
    if tables.is_empty() {
        eprintln!("Skipping column test: no tables in public schema");
        return;
    }

    let qualified = format!("public.{}", tables[0].name);
    let columns = provider
        .get_columns(&qualified)
        .await
        .expect("get_columns failed");
    assert!(!columns.is_empty(), "table should have at least one column");
    for col in &columns {
        assert!(!col.name.is_empty(), "column name should not be empty");
    }
}

#[tokio::test]
async fn test_postgres_get_indexes() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let tables = provider
        .get_tables("public")
        .await
        .expect("get_tables failed");
    if tables.is_empty() {
        eprintln!("Skipping index test: no tables in public schema");
        return;
    }

    let qualified = format!("public.{}", tables[0].name);
    let indexes = provider.get_indexes(&qualified).await;
    assert!(indexes.is_ok(), "get_indexes failed: {:?}", indexes.err());
    // Indexes may be empty, that's fine
}

#[tokio::test]
async fn test_postgres_get_views() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let _views = provider
        .get_views("public")
        .await
        .expect("get_views failed");
    // Views may be empty, that's fine
}

#[tokio::test]
async fn test_postgres_get_extensions() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let _extensions = provider
        .get_extensions()
        .await
        .expect("get_extensions failed");
    // Extensions may be empty, that's fine
}

#[tokio::test]
async fn test_postgres_disconnect() {
    let config = match get_config() {
        Some(c) => c,
        None => {
            eprintln!("Skipping: set DATABASE_USERNAME and DATABASE_PASSWORD");
            return;
        }
    };

    let mut provider = PostgresProvider::new();
    provider.connect(&config).await.expect("connect failed");

    let result = provider.disconnect().await;
    assert!(result.is_ok(), "disconnect failed: {:?}", result.err());

    // After disconnect, ping should fail
    let ping = provider.ping().await;
    assert!(ping.is_err(), "ping should fail after disconnect");
}

// ── Helpers ────────────────────────────────────────────────────────

fn table_name(prefix: &str) -> String {
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    format!("test_{}_{}", prefix, suffix)
}

fn skip_if_no_config() -> Option<ConnectionConfig> {
    let c = get_config()?;
    Some(c)
}

async fn ensure_provider(config: &ConnectionConfig) -> PostgresProvider {
    let mut p = PostgresProvider::new();
    p.connect(config).await.expect("connect failed");
    p
}

// ── DDL: CREATE / ALTER / DROP TABLE ──────────────────────────────

#[tokio::test]
async fn test_postgres_create_table_all_types() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("all_types");

    let sql = format!(
        r#"CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            v_int INTEGER,
            v_bigint BIGINT,
            v_smallint SMALLINT,
            v_numeric NUMERIC(10,2),
            v_real REAL,
            v_double DOUBLE PRECISION,
            v_text TEXT,
            v_varchar VARCHAR(100),
            v_char CHAR(1),
            v_bool BOOLEAN,
            v_date DATE,
            v_time TIME,
            v_timestamp TIMESTAMP,
            v_timestamptz TIMESTAMPTZ,
            v_json JSON,
            v_jsonb JSONB,
            v_bytea BYTEA,
            v_uuid UUID
        )"#,
        tbl
    );
    let r = provider.execute_query(&sql).await;
    assert!(r.is_ok(), "CREATE TABLE failed: {:?}", r.err());

    // Verify table exists via get_tables
    let tables = provider.get_tables("public").await.unwrap();
    assert!(
        tables.iter().any(|t| t.name == tbl),
        "table should appear in get_tables"
    );

    // Verify columns via get_columns
    let qualified = format!("public.{}", tbl);
    let cols = provider.get_columns(&qualified).await.unwrap();
    assert!(
        cols.len() >= 19,
        "should have 19+ columns, got {}",
        cols.len()
    );

    let col_names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
    for name in &["v_int", "v_text", "v_bool", "v_jsonb", "v_uuid"] {
        assert!(col_names.contains(name), "missing column {}", name);
    }

    // Clean up
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_alter_and_drop_table() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("alter_drop");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
            tbl
        ))
        .await
        .unwrap();

    // ALTER TABLE ADD COLUMN
    let r = provider
        .execute_query(&format!("ALTER TABLE {} ADD COLUMN age INTEGER", tbl))
        .await;
    assert!(r.is_ok(), "ALTER ADD COLUMN failed: {:?}", r.err());

    let cols = provider
        .get_columns(&format!("public.{}", tbl))
        .await
        .unwrap();
    assert!(
        cols.iter().any(|c| c.name == "age"),
        "age column should exist"
    );

    // DROP TABLE
    let r = provider.execute_query(&format!("DROP TABLE {}", tbl)).await;
    assert!(r.is_ok(), "DROP TABLE failed: {:?}", r.err());

    // Verify dropped
    let tables = provider.get_tables("public").await.unwrap();
    assert!(
        !tables.iter().any(|t| t.name == tbl),
        "table should not exist after DROP"
    );
}

// ── INSERT operations ──────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_insert_single_row() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("insert_single");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, val INTEGER)",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "INSERT INTO {} (name, val) VALUES ('alice', 100)",
            tbl
        ))
        .await
        .unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 1);

    // Verify
    let r = provider
        .execute_query(&format!("SELECT name, val FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.col_count(), 2);
    assert_eq!(r.rows[0][0].to_string(), "alice");
    assert_eq!(r.rows[0][1].to_string(), "100");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_insert_multi_row() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("insert_multi");

    provider
        .execute_query(&format!("CREATE TABLE {} (x INTEGER, y INTEGER)", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "INSERT INTO {} (x, y) VALUES (1,10), (2,20), (3,30)",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 3);

    let r = provider
        .execute_query(&format!("SELECT COUNT(*) AS cnt FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "3");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_insert_returning() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("insert_ret");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, label TEXT)",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "INSERT INTO {} (label) VALUES ('hello') RETURNING id, label",
            tbl
        ))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.col_count(), 2);
    assert_eq!(r.rows[0][1].to_string(), "hello");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

// ── SELECT operations ──────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_select_where_operators() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_where");

    let create = format!(
        "CREATE TABLE {} (id SERIAL, category TEXT, score INTEGER, active BOOLEAN)",
        tbl
    );
    provider.execute_query(&create).await.unwrap();
    provider.execute_query(&format!("INSERT INTO {} (category, score, active) VALUES ('A',10,true), ('B',20,false), ('A',30,true), ('C',40,true)", tbl)).await.unwrap();

    // =, <, >, <=, >=, <>
    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE category = 'A' ORDER BY score",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);

    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE score > 20 ORDER BY score",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);

    let r = provider
        .execute_query(&format!("SELECT score FROM {} WHERE score <> 20", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 3);

    // IN
    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE category IN ('A','C') ORDER BY score",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 3);

    // BETWEEN
    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE score BETWEEN 20 AND 40",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 3);

    // LIKE
    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE category LIKE 'A'",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);

    // IS NULL / IS NOT NULL
    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE active IS NOT NULL",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 4);

    // AND / OR / NOT
    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE category = 'A' AND score >= 20",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);

    let r = provider
        .execute_query(&format!(
            "SELECT score FROM {} WHERE category = 'A' OR category = 'B'",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 3);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_aggregates() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_agg");

    provider
        .execute_query(&format!("CREATE TABLE {} (g TEXT, v INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (g, v) VALUES ('A',10), ('A',20), ('B',5), ('B',15)",
            tbl
        ))
        .await
        .unwrap();

    // COUNT, SUM, AVG, MIN, MAX
    let r = provider.execute_query(&format!("SELECT COUNT(*) AS cnt, SUM(v) AS total, AVG(v) AS avg, MIN(v) AS mn, MAX(v) AS mx FROM {}", tbl)).await.unwrap();
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][0].to_string(), "4");
    assert_eq!(r.rows[0][1].to_string(), "50");
    assert_eq!(r.rows[0][3].to_string(), "5");
    assert_eq!(r.rows[0][4].to_string(), "20");

    // GROUP BY + HAVING
    let r = provider
        .execute_query(&format!(
            "SELECT g, SUM(v) AS total FROM {} GROUP BY g HAVING SUM(v) > 20",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][0].to_string(), "A");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_order_limit_offset() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_order");

    provider
        .execute_query(&format!("CREATE TABLE {} (n INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (n) VALUES (3), (1), (4), (2)",
            tbl
        ))
        .await
        .unwrap();

    // ORDER BY ASC
    let r = provider
        .execute_query(&format!("SELECT n FROM {} ORDER BY n ASC", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 4);
    assert_eq!(r.rows[0][0].to_string(), "1");
    assert_eq!(r.rows[3][0].to_string(), "4");

    // ORDER BY DESC
    let r = provider
        .execute_query(&format!("SELECT n FROM {} ORDER BY n DESC", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "4");

    // LIMIT
    let r = provider
        .execute_query(&format!("SELECT n FROM {} ORDER BY n LIMIT 2", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);

    // OFFSET
    let r = provider
        .execute_query(&format!("SELECT n FROM {} ORDER BY n OFFSET 2", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);
    assert_eq!(r.rows[0][0].to_string(), "3");

    // LIMIT + OFFSET
    let r = provider
        .execute_query(&format!(
            "SELECT n FROM {} ORDER BY n LIMIT 1 OFFSET 2",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][0].to_string(), "3");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_join() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let t1 = table_name("join_a");
    let t2 = table_name("join_b");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
            t1
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, a_id INTEGER REFERENCES {}(id), val TEXT)",
            t2, t1
        ))
        .await
        .unwrap();

    provider
        .execute_query(&format!(
            "INSERT INTO {} (name) VALUES ('Alice'), ('Bob')",
            t1
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (a_id, val) VALUES (1, 'X'), (1, 'Y'), (2, 'Z')",
            t2
        ))
        .await
        .unwrap();

    // INNER JOIN
    let sql = format!(
        "SELECT a.name, b.val FROM {} a JOIN {} b ON a.id = b.a_id ORDER BY a.name, b.val",
        t1, t2
    );
    let r = provider.execute_query(&sql).await.unwrap();
    assert_eq!(r.row_count(), 3);
    assert_eq!(r.col_count(), 2);

    // LEFT JOIN
    provider
        .execute_query(&format!("INSERT INTO {} (name) VALUES ('Charlie')", t1))
        .await
        .unwrap();
    let sql = format!(
        "SELECT a.name, b.val FROM {} a LEFT JOIN {} b ON a.id = b.a_id ORDER BY a.name",
        t1, t2
    );
    let r = provider.execute_query(&sql).await.unwrap();
    assert_eq!(r.row_count(), 4);

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
async fn test_postgres_select_distinct() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_distinct");

    provider
        .execute_query(&format!("CREATE TABLE {} (color TEXT)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (color) VALUES ('red'), ('blue'), ('red'), ('green'), ('blue')",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "SELECT DISTINCT color FROM {} ORDER BY color",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 3);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_subquery() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_subq");

    provider
        .execute_query(&format!("CREATE TABLE {} (dept TEXT, salary INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (dept, salary) VALUES ('ENG',100), ('ENG',200), ('MKT',150)",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "SELECT dept, salary FROM {} WHERE salary > (SELECT AVG(salary) FROM {}) ORDER BY dept",
            tbl, tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_cte() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_cte");

    provider
        .execute_query(&format!("CREATE TABLE {} (x INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (x) VALUES (1), (2), (3)", tbl))
        .await
        .unwrap();

    let sql = format!(
        "WITH doubled AS (SELECT x, x * 2 AS x2 FROM {}) SELECT x2 FROM doubled WHERE x >= 2 ORDER BY x2", tbl
    );
    let r = provider.execute_query(&sql).await.unwrap();
    assert_eq!(r.row_count(), 2);
    assert_eq!(r.rows[0][0].to_string(), "4");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_table_command() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("sel_table_cmd");

    provider
        .execute_query(&format!("CREATE TABLE {} (v INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (v) VALUES (10), (20)", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("TABLE {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 2);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_select_explain() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;

    let r = provider.execute_query("EXPLAIN SELECT 1").await.unwrap();
    assert!(r.is_select);
    assert!(r.row_count() >= 1);
}

// ── UPDATE operations ──────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_update_single_row() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("upd_single");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('old')", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("UPDATE {} SET val = 'new' WHERE id = 1", tbl))
        .await
        .unwrap();
    assert!(!r.is_select);
    assert_eq!(r.rows_affected, 1);

    let r = provider
        .execute_query(&format!("SELECT val FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "new");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_update_multi_row() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("upd_multi");

    provider
        .execute_query(&format!("CREATE TABLE {} (cat TEXT, val INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (cat, val) VALUES ('A',1), ('A',2), ('B',3)",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "UPDATE {} SET val = val * 10 WHERE cat = 'A'",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 2);

    let r = provider
        .execute_query(&format!(
            "SELECT val FROM {} WHERE cat = 'A' ORDER BY val",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "10");
    assert_eq!(r.rows[1][0].to_string(), "20");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_update_returning() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("upd_ret");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, label TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (label) VALUES ('orig')", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "UPDATE {} SET label = 'updated' WHERE id = 1 RETURNING id, label",
            tbl
        ))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][1].to_string(), "updated");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_update_zero_rows() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("upd_zero");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (k INTEGER PRIMARY KEY, v INTEGER)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (k, v) VALUES (1, 100)", tbl))
        .await
        .unwrap();

    // Update with no match
    let r = provider
        .execute_query(&format!("UPDATE {} SET v = 999 WHERE k = 999", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 0);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

// ── DELETE operations ──────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_delete_single_row() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("del_single");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name) VALUES ('delme'), ('keep')",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("DELETE FROM {} WHERE name = 'delme'", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 1);

    let r = provider
        .execute_query(&format!("SELECT COUNT(*) FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "1");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_delete_returning() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("del_ret");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('gone')", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!(
            "DELETE FROM {} WHERE id = 1 RETURNING id, val",
            tbl
        ))
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][1].to_string(), "gone");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_delete_all_rows() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("del_all");

    provider
        .execute_query(&format!("CREATE TABLE {} (x INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (x) VALUES (1), (2), (3)", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("DELETE FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 3);

    let r = provider
        .execute_query(&format!("SELECT COUNT(*) FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "0");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

// ── Data type tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_data_types_insert_select() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("data_types");

    let create = format!(
        r#"CREATE TABLE {} (
        id SERIAL PRIMARY KEY,
        v_int INTEGER, v_bigint BIGINT, v_smallint SMALLINT,
        v_dec NUMERIC(10,2), v_float REAL,
        v_text TEXT, v_vc VARCHAR(50),
        v_bool BOOLEAN,
        v_json JSONB,
        v_bytea BYTEA
    )"#,
        tbl
    );
    provider.execute_query(&create).await.unwrap();

    let insert = format!(
        r#"INSERT INTO {} (v_int, v_bigint, v_smallint, v_dec, v_float, v_text, v_vc, v_bool, v_json, v_bytea)
        VALUES (42, 9999999999, 127, 123.45, 3.14, 'hello', 'vc', true, '{{"k":"v"}}'::JSONB, E'\\xdeadbeef'::BYTEA)"#,
        tbl
    );
    provider.execute_query(&insert).await.unwrap();

    let r = provider
        .execute_query(&format!(
            "SELECT v_int, v_bigint, v_smallint, v_dec, v_float, v_text, v_vc, v_bool FROM {}",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);

    // Check types and values
    match &r.rows[0][0] {
        dbrusty::database::result::Value::Integer(v) => assert_eq!(*v, 42),
        _ => panic!("expected Integer"),
    }
    match &r.rows[0][1] {
        dbrusty::database::result::Value::Integer(v) => assert_eq!(*v, 9999999999),
        _ => panic!("expected Integer"),
    }
    match &r.rows[0][2] {
        dbrusty::database::result::Value::Integer(v) => assert_eq!(*v, 127),
        _ => panic!("expected Integer"),
    }
    match &r.rows[0][4] {
        dbrusty::database::result::Value::Float(v) => assert!(*v > 3.0),
        _ => panic!("expected Float"),
    }
    assert_eq!(r.rows[0][5].to_string(), "hello");
    assert_eq!(r.rows[0][6].to_string(), "vc");
    match &r.rows[0][7] {
        dbrusty::database::result::Value::Bool(v) => assert_eq!(*v, true),
        _ => panic!("expected Bool"),
    }

    // Verify JSON column exists
    let cols = provider
        .get_columns(&format!("public.{}", tbl))
        .await
        .unwrap();
    assert!(
        cols.iter().any(|c| c.name == "v_json"),
        "json column should exist"
    );

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_null_handling() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("null_test");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, required TEXT NOT NULL, optional TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (required, optional) VALUES ('has', NULL)",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("SELECT required, optional FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "has");
    match &r.rows[0][1] {
        dbrusty::database::result::Value::Null => {}
        _ => panic!("expected Null for optional"),
    }

    // IS NULL query
    let r = provider
        .execute_query(&format!(
            "SELECT COUNT(*) FROM {} WHERE optional IS NULL",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "1");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_empty_result() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;

    let r = provider
        .execute_query("SELECT * FROM (VALUES (1)) AS t WHERE false")
        .await
        .unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 0);
}

// ── DDL: Index, View, Schema ──────────────────────────────────────

#[tokio::test]
async fn test_postgres_create_index() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("idx_test");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, email TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE INDEX idx_{}_email ON {} (email)",
            tbl, tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (email) VALUES ('a@b.com'), ('c@d.com')",
            tbl
        ))
        .await
        .unwrap();

    // Verify index via get_indexes
    let indexes = provider
        .get_indexes(&format!("public.{}", tbl))
        .await
        .unwrap();
    assert!(
        indexes.iter().any(|idx| idx.name.contains("email")),
        "email index should exist"
    );

    // Verify the index actually works (seq scan vs index scan not checked, just correctness)
    let r = provider
        .execute_query(&format!(
            "SELECT email FROM {} WHERE email = 'a@b.com'",
            tbl
        ))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_create_view() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("view_base");
    let view = table_name("test_view");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, active BOOLEAN)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (name, active) VALUES ('A',true), ('B',false)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "CREATE VIEW {} AS SELECT name FROM {} WHERE active = true",
            view, tbl
        ))
        .await
        .unwrap();

    // Verify via get_views
    let views = provider.get_views("public").await.unwrap();
    assert!(
        views.iter().any(|v| v.name == view),
        "view should appear in get_views"
    );

    // Query the view
    let r = provider
        .execute_query(&format!("SELECT name FROM {} ORDER BY name", view))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][0].to_string(), "A");

    provider
        .execute_query(&format!("DROP VIEW {}", view))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_create_schema() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let schema = format!("test_schema_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // CREATE SCHEMA
    let r = provider
        .execute_query(&format!("CREATE SCHEMA {}", schema))
        .await;
    assert!(r.is_ok(), "CREATE SCHEMA failed: {:?}", r.err());

    // Verify via get_schemas
    let schemas = provider.get_schemas().await.unwrap();
    assert!(
        schemas.contains(&schema),
        "schema should appear in get_schemas"
    );

    // Create a table in that schema and list it
    provider
        .execute_query(&format!("CREATE TABLE {}.my_table (id INTEGER)", schema))
        .await
        .unwrap();
    let tables = provider.get_tables(&schema).await.unwrap();
    assert!(
        tables.iter().any(|t| t.name == "my_table"),
        "table in new schema should appear"
    );

    // Verify get_columns works with schema-qualified name
    let cols = provider
        .get_columns(&format!("{}.my_table", schema))
        .await
        .unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "id");

    // Cleanup
    provider
        .execute_query(&format!("DROP TABLE {}.my_table", schema))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP SCHEMA {}", schema))
        .await
        .unwrap();
}

// ── Transaction tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_transaction_commit() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_commit");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();

    provider.begin_transaction().await.unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('committed')", tbl))
        .await
        .unwrap();
    provider.commit().await.unwrap();

    // Verify data persisted after commit
    let r = provider
        .execute_query(&format!("SELECT val FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_transaction_rollback() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_rollback");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();

    provider.begin_transaction().await.unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('rolled_back')", tbl))
        .await
        .unwrap();
    provider.rollback().await.unwrap();

    // Verify data NOT persisted after rollback
    let r = provider
        .execute_query(&format!("SELECT COUNT(*) FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "0");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_transaction_savepoint() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("tx_svp");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();

    provider.begin_transaction().await.unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('before_svp')", tbl))
        .await
        .unwrap();
    provider.execute_query("SAVEPOINT sp1").await.unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('after_svp')", tbl))
        .await
        .unwrap();
    provider
        .execute_query("ROLLBACK TO SAVEPOINT sp1")
        .await
        .unwrap();
    provider.commit().await.unwrap();

    // after_svp should be rolled back, before_svp should persist
    let r = provider
        .execute_query(&format!("SELECT val FROM {} ORDER BY id", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 1);
    assert_eq!(r.rows[0][0].to_string(), "before_svp");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

// ── Metadata: Sequences, Functions, Procedures ─────────────────────

#[tokio::test]
async fn test_postgres_sequence_operations() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("seq_tbl");
    let seq = format!("{}_my_seq", tbl);

    provider
        .execute_query(&format!("CREATE TABLE {} (id INTEGER)", tbl))
        .await
        .unwrap();

    // CREATE SEQUENCE and use it
    let r = provider
        .execute_query(&format!("CREATE SEQUENCE {}", seq))
        .await;
    assert!(r.is_ok(), "CREATE SEQUENCE failed: {:?}", r.err());

    provider
        .execute_query(&format!(
            "INSERT INTO {} (id) VALUES (NEXTVAL('{}'))",
            tbl, seq
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (id) VALUES (NEXTVAL('{}'))",
            tbl, seq
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("SELECT id FROM {} ORDER BY id", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "1");
    assert_eq!(r.rows[1][0].to_string(), "2");

    // Verify via get_sequences
    let seqs = provider.get_sequences("public").await.unwrap();
    assert!(
        seqs.iter().any(|s| s.name == seq),
        "sequence should appear in get_sequences"
    );

    provider
        .execute_query(&format!("DROP SEQUENCE {}", seq))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_function_operations() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let fn_name = format!("test_func_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // Create a simple SQL function
    let sql = format!(
        "CREATE FUNCTION {}(a INTEGER, b INTEGER) RETURNS INTEGER
         LANGUAGE SQL IMMUTABLE
         AS $$ SELECT a + b $$",
        fn_name
    );
    let r = provider.execute_query(&sql).await;
    assert!(r.is_ok(), "CREATE FUNCTION failed: {:?}", r.err());

    // Call the function
    let _ = provider
        .execute_query(&format!("SELECT {}()", fn_name))
        .await;
    let r = provider
        .execute_query(&format!("SELECT {}(1, 2) AS result", fn_name))
        .await;
    assert!(r.is_ok(), "calling function failed: {:?}", r.err());
    if r.is_ok() {
        let r = r.unwrap();
        assert_eq!(r.rows[0][0].to_string(), "3");
    }

    // Verify via get_functions
    let funcs = provider.get_functions("public").await.unwrap();
    assert!(
        funcs.iter().any(|f| f.name == fn_name),
        "function should appear in get_functions"
    );

    provider
        .execute_query(&format!("DROP FUNCTION {}", fn_name))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_procedure_operations() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("proc_tbl");
    let proc_name = format!("test_proc_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();

    // Create a procedure
    let sql = format!(
        "CREATE PROCEDURE {}(p_val TEXT)
         LANGUAGE SQL
         AS $$ INSERT INTO {} (val) VALUES (p_val) $$",
        proc_name, tbl
    );
    let r = provider.execute_query(&sql).await;
    assert!(r.is_ok(), "CREATE PROCEDURE failed: {:?}", r.err());

    // Call the procedure via CALL
    let r = provider
        .execute_query(&format!("CALL {}('hello')", proc_name))
        .await;
    assert!(r.is_ok(), "CALL failed: {:?}", r.err());

    // Verify the insert happened
    let r = provider
        .execute_query(&format!("SELECT val FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "hello");

    // Verify via get_procedures
    let procs = provider.get_procedures("public").await.unwrap();
    assert!(
        procs.iter().any(|p| p.name == proc_name),
        "procedure should appear in get_procedures"
    );

    provider
        .execute_query(&format!("DROP PROCEDURE {}", proc_name))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_postgres_trigger_operations() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("trig_tbl");
    let fn_name = format!("trig_fn_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let trig_name = format!("trig_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT, log TEXT)",
            tbl
        ))
        .await
        .unwrap();

    // Create trigger function
    provider
        .execute_query(&format!(
            "CREATE FUNCTION {}() RETURNS TRIGGER
         LANGUAGE PLPGSQL AS $$ BEGIN NEW.log := 'triggered'; RETURN NEW; END $$",
            fn_name
        ))
        .await
        .unwrap();

    // Create trigger
    provider
        .execute_query(&format!(
            "CREATE TRIGGER {} BEFORE INSERT ON {}
         FOR EACH ROW EXECUTE FUNCTION {}()",
            trig_name, tbl, fn_name
        ))
        .await
        .unwrap();

    // Insert to trigger
    provider
        .execute_query(&format!("INSERT INTO {} (val) VALUES ('test')", tbl))
        .await
        .unwrap();

    // Verify trigger worked
    let r = provider
        .execute_query(&format!("SELECT val, log FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "test");
    assert_eq!(r.rows[0][1].to_string(), "triggered");

    // Verify via get_triggers
    let triggers = provider.get_triggers("public").await.unwrap();
    assert!(
        triggers.iter().any(|t| t.name == trig_name),
        "trigger should appear in get_triggers"
    );

    provider
        .execute_query(&format!("DROP TRIGGER {} ON {}", trig_name, tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP FUNCTION {}", fn_name))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

// ── Metadata: Custom types ─────────────────────────────────────────

#[tokio::test]
async fn test_postgres_custom_types() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let enum_name = format!("test_enum_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // CREATE ENUM type
    provider
        .execute_query(&format!(
            "CREATE TYPE {} AS ENUM ('small', 'medium', 'large')",
            enum_name
        ))
        .await
        .unwrap();

    // Verify via get_custom_types
    let types = provider.get_custom_types("public").await.unwrap();
    assert!(
        types.iter().any(|t| t.name == enum_name),
        "custom enum should appear in get_custom_types"
    );

    // Use the enum in a table
    let tbl = table_name("enum_tbl");
    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, size {})",
            tbl, enum_name
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!("INSERT INTO {} (size) VALUES ('medium')", tbl))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("SELECT size::TEXT FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "medium");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!("DROP TYPE {}", enum_name))
        .await
        .unwrap();
}

// ── EXPLAIN / ANALYZE ──────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_explain_analyze() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;

    let r = provider
        .execute_query("EXPLAIN ANALYZE SELECT 1 AS val")
        .await
        .unwrap();
    assert!(r.is_select);
    assert!(r.row_count() >= 1);
}

// ── SHOW commands ──────────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_show_commands() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;

    let r = provider.execute_query("SHOW server_version").await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);

    let r = provider.execute_query("SHOW search_path").await.unwrap();
    assert!(r.is_select);
    assert_eq!(r.row_count(), 1);
}

// ── Large result set ───────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_select_100_rows() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("large_sel");

    provider
        .execute_query(&format!("CREATE TABLE {} (n INTEGER)", tbl))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (n) SELECT generate_series(1, 100)",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("SELECT COUNT(*) FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "100");

    let r = provider
        .execute_query(&format!("SELECT n FROM {} ORDER BY n", tbl))
        .await
        .unwrap();
    assert_eq!(r.row_count(), 100);

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}

// ── DDL: TRUNCATE ──────────────────────────────────────────────────

#[tokio::test]
async fn test_postgres_truncate() {
    let config = match skip_if_no_config() {
        Some(c) => c,
        None => return,
    };
    let provider = ensure_provider(&config).await;
    let tbl = table_name("trunc_tbl");

    provider
        .execute_query(&format!(
            "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
            tbl
        ))
        .await
        .unwrap();
    provider
        .execute_query(&format!(
            "INSERT INTO {} (val) VALUES ('a'), ('b'), ('c')",
            tbl
        ))
        .await
        .unwrap();

    let r = provider
        .execute_query(&format!("TRUNCATE {}", tbl))
        .await
        .unwrap();
    assert!(!r.is_select);

    let r = provider
        .execute_query(&format!("SELECT COUNT(*) FROM {}", tbl))
        .await
        .unwrap();
    assert_eq!(r.rows[0][0].to_string(), "0");

    provider
        .execute_query(&format!("DROP TABLE {}", tbl))
        .await
        .unwrap();
}
