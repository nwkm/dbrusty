use crate::completion::provider::CompletionProvider;
use crate::completion::sql_keywords;
use crate::completion::{CompletionItem, CompletionKind};
use crate::database::metadata::MetadataCache;
use crate::models::connection::DatabaseType;

pub struct SqlCompletionProvider;

impl CompletionProvider for SqlCompletionProvider {
    fn completions(
        &self,
        metadata: &MetadataCache,
        db_type: Option<&DatabaseType>,
    ) -> Vec<CompletionItem> {
        let mut items: Vec<CompletionItem> = Vec::new();

        for kw in sql_keywords::KEYWORDS {
            items.push(CompletionItem::new(kw, CompletionKind::Keyword));
        }

        for t in sql_keywords::TYPES {
            items.push(CompletionItem::new(t, CompletionKind::Type));
        }

        for f in sql_keywords::FUNCTIONS {
            items.push(CompletionItem::new(f, CompletionKind::Function));
        }

        match db_type {
            Some(DatabaseType::Postgres) => {
                for f in sql_keywords::POSTGRES_FUNCTIONS {
                    items.push(CompletionItem::new(f, CompletionKind::Function));
                }
            }
            Some(DatabaseType::MySQL) | Some(DatabaseType::MariaDB) => {
                for f in sql_keywords::MYSQL_FUNCTIONS {
                    items.push(CompletionItem::new(f, CompletionKind::Function));
                }
            }
            _ => {}
        }

        for (_schema_key, tables) in &metadata.tables {
            for table in tables {
                items.push(CompletionItem::with_detail(
                    &table.name,
                    CompletionKind::Table,
                    &format!("{} ({})", table.schema, table.table_type),
                ));
            }
        }

        for (table_key, columns) in &metadata.columns {
            for col in columns {
                let detail = if col.is_pk {
                    format!("{}.{} PK", table_key, col.type_name)
                } else if col.is_fk {
                    format!("{}.{} FK", table_key, col.type_name)
                } else {
                    format!("{}.{}", table_key, col.type_name)
                };
                items.push(CompletionItem::with_detail(
                    &col.name,
                    CompletionKind::Column,
                    &detail,
                ));
            }
        }

        for (_schema_key, views) in &metadata.views {
            for view in views {
                items.push(CompletionItem::with_detail(
                    &view.name,
                    CompletionKind::View,
                    &view.schema,
                ));
            }
        }

        items
    }
}
