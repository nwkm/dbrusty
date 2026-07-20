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
