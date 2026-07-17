use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DatabaseType {
    #[serde(rename = "postgres")]
    Postgres,
    #[serde(rename = "mysql")]
    MySQL,
    #[serde(rename = "mariadb")]
    MariaDB,
    #[serde(rename = "sqlite")]
    SQLite,
    #[serde(rename = "sqlserver")]
    SQLServer,
    #[serde(rename = "mongodb")]
    MongoDB,
    #[serde(rename = "redis")]
    Redis,
}

impl DatabaseType {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "postgres",
            DatabaseType::MySQL => "mysql",
            DatabaseType::MariaDB => "mariadb",
            DatabaseType::SQLite => "sqlite",
            DatabaseType::SQLServer => "sqlserver",
            DatabaseType::MongoDB => "mongodb",
            DatabaseType::Redis => "redis",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            DatabaseType::Postgres => 5432,
            DatabaseType::MySQL | DatabaseType::MariaDB => 3306,
            DatabaseType::SQLite => 0,
            DatabaseType::SQLServer => 1433,
            DatabaseType::MongoDB => 27017,
            DatabaseType::Redis => 6379,
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "PostgreSQL",
            DatabaseType::MySQL => "MySQL",
            DatabaseType::MariaDB => "MariaDB",
            DatabaseType::SQLite => "SQLite",
            DatabaseType::SQLServer => "SQL Server",
            DatabaseType::MongoDB => "MongoDB",
            DatabaseType::Redis => "Redis",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: Uuid,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub group: Option<String>,
    pub is_favorite: bool,
    pub ssl: bool,
    pub created_at: DateTime<Utc>,
    pub last_connected: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub password: Option<String>,
}

#[allow(dead_code)]
impl ConnectionConfig {
    pub fn new(name: String, db_type: DatabaseType) -> Self {
        let port = db_type.default_port();
        Self {
            id: Uuid::new_v4(),
            name,
            db_type,
            host: "localhost".to_string(),
            port,
            database: String::new(),
            username: String::new(),
            group: None,
            is_favorite: false,
            ssl: false,
            created_at: Utc::now(),
            last_connected: None,
            password: None,
        }
    }

    pub fn password_key(&self) -> String {
        format!("dbrusty/{}", self.id)
    }

    pub fn connection_string(&self, password: Option<&str>) -> Result<String> {
        match self.db_type {
            DatabaseType::Postgres => {
                let pw = password.unwrap_or("");
                Ok(format!(
                    "postgres://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                ))
            }
            DatabaseType::MySQL | DatabaseType::MariaDB => {
                let pw = password.unwrap_or("");
                Ok(format!(
                    "mysql://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                ))
            }
            DatabaseType::SQLite => Ok(format!("sqlite://{}", self.database)),
            DatabaseType::SQLServer => {
                let pw = password.unwrap_or("");
                Ok(format!(
                    "sqlserver://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                ))
            }
            DatabaseType::MongoDB => {
                let pw = password.unwrap_or("");
                Ok(format!(
                    "mongodb://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                ))
            }
            DatabaseType::Redis => {
                let pw = password.unwrap_or("");
                if pw.is_empty() {
                    Ok(format!("redis://{}:{}/0", self.host, self.port))
                } else {
                    Ok(format!("redis://:{}@{}:{}/0", pw, self.host, self.port))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionList {
    pub connections: Vec<ConnectionConfig>,
}
