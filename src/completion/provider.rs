use crate::completion::CompletionItem;
use crate::database::metadata::MetadataCache;
use crate::models::connection::DatabaseType;

pub trait CompletionProvider {
    fn completions(
        &self,
        metadata: &MetadataCache,
        db_type: Option<&DatabaseType>,
    ) -> Vec<CompletionItem>;
}
