use async_trait::async_trait;
use thiserror::Error;

use crate::database::result::QueryResult;

impl From<sqlx::Error> for DatabaseError {
    fn from(e: sqlx::Error) -> Self {
        DatabaseError::Execution(e.to_string())
    }
}
use crate::database::types::{
    ColumnInfo, FunctionInfo, IndexInfo, ProcedureInfo, SequenceInfo, ServerInfo, TableInfo,
    TriggerInfo, TypeSchemaInfo, ViewInfo,
};
use crate::models::connection::ConnectionConfig;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Query failed: {0}")]
    Execution(String),
    #[error("Not connected to database")]
    NotConnected,
    #[error("Metadata query failed: {0}")]
    Metadata(String),
    #[error("Transaction error: {0}")]
    Transaction(String),
}

pub type DatabaseResult<T> = std::result::Result<T, DatabaseError>;

#[async_trait]
pub trait DatabaseProvider: Send + Sync {
    async fn connect(&mut self, config: &ConnectionConfig) -> DatabaseResult<()>;
    async fn disconnect(&mut self) -> DatabaseResult<()>;
    async fn ping(&self) -> DatabaseResult<bool>;
    async fn execute_query(&self, sql: &str) -> DatabaseResult<QueryResult>;
    async fn get_databases(&self) -> DatabaseResult<Vec<String>>;
    async fn get_tables(&self, schema: &str) -> DatabaseResult<Vec<TableInfo>>;
    async fn get_columns(&self, table: &str) -> DatabaseResult<Vec<ColumnInfo>>;
    async fn get_indexes(&self, table: &str) -> DatabaseResult<Vec<IndexInfo>>;
    async fn get_views(&self, schema: &str) -> DatabaseResult<Vec<ViewInfo>>;
    async fn get_procedures(&self, schema: &str) -> DatabaseResult<Vec<ProcedureInfo>>;
    async fn begin_transaction(&self) -> DatabaseResult<()>;
    async fn commit(&self) -> DatabaseResult<()>;
    async fn rollback(&self) -> DatabaseResult<()>;
    async fn cancel_query(&self) -> DatabaseResult<()>;
    async fn server_info(&self) -> DatabaseResult<ServerInfo>;

    // PostgreSQL-specific metadata (default no-op for non-PG providers)
    async fn get_schemas(&self) -> DatabaseResult<Vec<String>> {
        Ok(vec!["public".to_string()])
    }

    async fn get_sequences(&self, _schema: &str) -> DatabaseResult<Vec<SequenceInfo>> {
        Ok(Vec::new())
    }

    async fn get_triggers(&self, _schema: &str) -> DatabaseResult<Vec<TriggerInfo>> {
        Ok(Vec::new())
    }

    async fn get_functions(&self, _schema: &str) -> DatabaseResult<Vec<FunctionInfo>> {
        Ok(Vec::new())
    }

    async fn get_extensions(&self) -> DatabaseResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn get_custom_types(&self, _schema: &str) -> DatabaseResult<Vec<TypeSchemaInfo>> {
        Ok(Vec::new())
    }
}
