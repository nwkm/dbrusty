/// Check what the explorer tree would show for a MongoDB connection.
/// Usage: cargo run --example check_explorer [database_name]

use dbrusty::database::provider::DatabaseProvider;
use dbrusty::models::connection::{ConnectionConfig, DatabaseType};
use dbrusty::providers::mongodb::MongoProvider;

#[tokio::main]
async fn main() {
    let db_name = std::env::args().nth(1).unwrap_or_else(|| "test".into());

    let config = ConnectionConfig {
        id: uuid::Uuid::new_v4(),
        name: "check".into(),
        db_type: DatabaseType::MongoDB,
        host: "127.0.0.1".into(),
        port: 27017,
        database: db_name.clone(),
        username: String::new(),
        group: None,
        is_favorite: false,
        ssl: false,
        created_at: chrono::Utc::now(),
        last_connected: None,
        password: None,
    };

    let mut provider = MongoProvider::new();
    provider.connect(&config).await.expect("connect failed");
    println!("Connected! database_name = '{}'", db_name);

    let databases = provider.get_databases().await.unwrap_or_default();
    println!("\nDatabases on server: {:?}", databases);

    println!("\n--- Explorer tree simulation ---");
    let mut tables_data: Vec<(String, Vec<dbrusty::database::types::TableInfo>)> = Vec::new();
    let mut schemas_data: Vec<(String, Vec<String>)> = Vec::new();

    for db in &databases {
        let mut schemas = provider.get_schemas().await.unwrap_or_default();
        // No explicit schemas (e.g. MongoDB) → use db name as synthetic schema
        if schemas.is_empty() {
            schemas = vec![db.clone()];
        }
        schemas_data.push((db.clone(), schemas.clone()));

        for schema in &schemas {
            let schema_key = format!("{}.{}", db, schema);
            match provider.get_tables(schema).await {
                Ok(tables) => {
                    println!("  db='{}', schema='{}' -> {} collections", db, schema, tables.len());
                    if !tables.is_empty() {
                        let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
                        println!("    collections: {:?}", names);
                    }
                    tables_data.push((schema_key, tables));
                }
                Err(e) => {
                    println!("  db='{}', schema='{}' -> ERROR: {}", db, schema, e);
                }
            }
        }
    }

    println!("\n--- Tree structure ---");
    for (db, schemas) in &schemas_data {
        let is_flat = schemas.len() == 1 && &schemas[0] == db;
        let flat_str = if is_flat { "FLAT" } else { "SCHEMA" };
        println!("  {} ({})", db, flat_str);
        for schema in schemas {
            let key = format!("{}.{}", db, schema);
            let count = tables_data.iter().find(|(k, _)| k == &key).map(|(_, t)| t.len()).unwrap_or(0);
            println!("    └─ {} ({} collections)", schema, count);
        }
    }
}
