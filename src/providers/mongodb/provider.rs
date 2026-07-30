use std::collections::BTreeMap;

use async_trait::async_trait;
use bson::{Bson, Document};
use futures_util::TryStreamExt;
use mongodb::Client;

use crate::database::provider::{DatabaseError, DatabaseProvider, DatabaseResult};
use crate::database::result::{QueryResult, Value};
use crate::database::types::*;
use crate::models::connection::ConnectionConfig;

pub struct MongoProvider {
    client: Option<Client>,
    database_name: String,
}

impl MongoProvider {
    pub fn new() -> Self {
        Self {
            client: None,
            database_name: String::new(),
        }
    }

    fn client(&self) -> DatabaseResult<&Client> {
        self.client.as_ref().ok_or(DatabaseError::NotConnected)
    }
}

fn bson_type_name(bson: &Bson) -> &'static str {
    match bson {
        Bson::Null => "null",
        Bson::Boolean(_) => "boolean",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Binary(_) => "binary",
        Bson::Decimal128(_) => "decimal128",
        Bson::RegularExpression(_) => "regex",
        Bson::Timestamp(_) => "timestamp",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascript",
        Bson::Document(_) => "object",
        Bson::Array(_) => "array",
        Bson::DbPointer(_) => "dbPointer",
        Bson::Symbol(_) => "symbol",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "maxKey",
        Bson::MinKey => "minKey",
    }
}

fn bson_to_value(bson: &Bson) -> Value {
    match bson {
        Bson::Null => Value::Null,
        Bson::Boolean(b) => Value::Bool(*b),
        Bson::Int32(i) => Value::Integer(*i as i64),
        Bson::Int64(i) => Value::Integer(*i),
        Bson::Double(f) => Value::Float(*f),
        Bson::String(s) => Value::Text(s.clone()),
        Bson::ObjectId(oid) => Value::Text(oid.to_hex()),
        Bson::DateTime(dt) => Value::Text(dt.to_string()),
        Bson::Timestamp(ts) => Value::Text(format!("{}:{}", ts.time, ts.increment)),
        Bson::Binary(bin) => Value::Blob(bin.bytes.clone()),
        Bson::Decimal128(d) => Value::Text(d.to_string()),
        Bson::RegularExpression(r) => {
            let pattern = r.pattern.to_string();
            let options = r.options.to_string();
            Value::Text(format!("/{}/{}", pattern, options))
        }
        Bson::JavaScriptCode(code) => Value::Text(code.clone()),
        Bson::JavaScriptCodeWithScope(cws) => Value::Text(cws.code.clone()),
        Bson::Symbol(s) => Value::Text(s.clone()),
        Bson::DbPointer(ptr) => Value::Text(format!("{:?}", ptr)),
        Bson::Document(d) => Value::Json(
            serde_json::to_string(&doc_to_json_value(d))
                .unwrap_or_else(|_| "<unserializable>".to_string()),
        ),
        Bson::Array(arr) => Value::Json(
            serde_json::to_string(
                &arr.iter()
                    .map(|v| bson_to_json_value(v))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "<unserializable>".to_string()),
        ),
        Bson::Undefined => Value::Text("undefined".to_string()),
        Bson::MaxKey => Value::Text("MaxKey".to_string()),
        Bson::MinKey => Value::Text("MinKey".to_string()),
    }
}

fn bson_to_json_value(bson: &Bson) -> serde_json::Value {
    match bson {
        Bson::Null => serde_json::Value::Null,
        Bson::Boolean(b) => serde_json::Value::Bool(*b),
        Bson::Int32(i) => serde_json::json!(*i),
        Bson::Int64(i) => serde_json::json!(*i),
        Bson::Double(f) => serde_json::json!(*f),
        Bson::String(s) => serde_json::Value::String(s.clone()),
        Bson::ObjectId(oid) => serde_json::json!({"$oid": oid.to_hex()}),
        Bson::DateTime(dt) => serde_json::json!({"$date": dt.to_string()}),
        Bson::Binary(bin) => {
            serde_json::json!({"$binary": {"base64": "", "subType": format!("{:02x}", bin.subtype)}})
        }
        Bson::Document(d) => doc_to_json_value(d),
        Bson::Array(arr) => serde_json::Value::Array(arr.iter().map(bson_to_json_value).collect()),
        _ => serde_json::Value::String(format!("{:?}", bson)),
    }
}

fn doc_to_json_value(doc: &Document) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in doc {
        map.insert(k.clone(), bson_to_json_value(v));
    }
    serde_json::Value::Object(map)
}

fn collect_fields(doc: &Document, prefix: &str, fields: &mut BTreeMap<String, (String, bool)>) {
    for (key, value) in doc {
        let full_path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        let is_pk = full_path == "_id" || full_path.ends_with("._id");
        let type_name = bson_type_name(value);

        let entry = fields
            .entry(full_path.clone())
            .or_insert_with(|| (type_name.to_string(), is_pk));
        if entry.0 != type_name {
            entry.0 = combine_types(&entry.0, type_name);
        }

        if let Some(subdoc) = value.as_document() {
            collect_fields(subdoc, &full_path, fields);
        }

        if let Some(arr) = value.as_array() {
            if let Some(first) = arr.first() {
                let elem_type = bson_type_name(first);
                let arr_type = format!("array<{}>", elem_type);
                if let Some(entry) = fields.get_mut(&full_path) {
                    if entry.0 != arr_type && !entry.0.starts_with("array<") {
                        entry.0 = arr_type.clone();
                    }
                }

                if let Some(subdoc) = first.as_document() {
                    let arr_prefix = format!("{}[]", full_path);
                    collect_fields(subdoc, &arr_prefix, fields);
                }
            }
        }
    }
}

fn combine_types(existing: &str, new: &str) -> String {
    if existing == new {
        return existing.to_string();
    }
    if let Some(inner) = existing
        .strip_prefix("mixed(")
        .and_then(|s| s.strip_suffix(')'))
    {
        if !inner.contains(new) {
            let mut parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            parts.push(new);
            parts.sort();
            format!("mixed({})", parts.join(", "))
        } else {
            existing.to_string()
        }
    } else {
        let mut parts = vec![existing, new];
        parts.sort();
        format!("mixed({})", parts.join(", "))
    }
}

fn json_to_bson(value: &serde_json::Value) -> DatabaseResult<Bson> {
    Bson::try_from(value.clone())
        .map_err(|e| DatabaseError::Execution(format!("JSON to BSON conversion failed: {}", e)))
}

fn json_to_doc(value: &serde_json::Value) -> DatabaseResult<Document> {
    match json_to_bson(value)? {
        Bson::Document(d) => Ok(d),
        _ => Err(DatabaseError::Execution(
            "Expected a JSON object (document)".to_string(),
        )),
    }
}

fn json_to_docs(value: &serde_json::Value) -> DatabaseResult<Vec<Document>> {
    let arr = value
        .as_array()
        .ok_or_else(|| DatabaseError::Execution("Expected a JSON array".to_string()))?;
    arr.iter().map(json_to_doc).collect()
}

fn docs_to_columns(docs: &[Document]) -> Vec<ColumnInfo> {
    let mut all_fields: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for doc in docs {
        collect_fields(doc, "", &mut all_fields);
    }

    if all_fields.is_empty() {
        return vec![ColumnInfo {
            name: "_id".to_string(),
            ordinal: 0,
            type_name: "objectId".to_string(),
            nullable: true,
            is_pk: true,
            is_fk: false,
            default: None,
            is_auto_increment: false,
        }];
    }

    all_fields
        .into_iter()
        .enumerate()
        .map(|(i, (name, (type_name, is_pk)))| ColumnInfo {
            name,
            ordinal: i,
            type_name,
            nullable: true,
            is_pk,
            is_fk: false,
            default: None,
            is_auto_increment: false,
        })
        .collect()
}

fn doc_to_row(doc: &Document, columns: &[ColumnInfo]) -> Vec<Value> {
    columns
        .iter()
        .map(|col| {
            let parts: Vec<&str> = col.name.split('.').collect();
            let mut current_value: Option<&Bson> = None;
            let mut current_doc = doc;

            #[allow(unused_assignments)]
            let mut owned_temp: Option<Bson> = None;

            for (i, part) in parts.iter().enumerate() {
                if part.contains("[]") {
                    let clean = part.trim_end_matches("[]");
                    match current_doc.get(clean) {
                        Some(Bson::Array(arr)) => {
                            return Value::Text(format!("[{} elements]", arr.len()));
                        }
                        Some(val) => {
                            current_value = Some(val);
                            break;
                        }
                        None => return Value::Null,
                    }
                }

                match current_doc.get(*part) {
                    Some(Bson::Document(subdoc)) => {
                        if i == parts.len() - 1 {
                            owned_temp = Some(Bson::Document(subdoc.clone()));
                            current_value = owned_temp.as_ref();
                        } else {
                            current_doc = subdoc;
                        }
                    }
                    Some(val) => {
                        current_value = Some(val);
                        break;
                    }
                    None => return Value::Null,
                }
            }

            match current_value {
                Some(val) => bson_to_value(val),
                None => {
                    if current_doc.is_empty() {
                        Value::Null
                    } else {
                        bson_to_value(&Bson::Document(current_doc.clone()))
                    }
                }
            }
        })
        .collect()
}

#[async_trait]
impl DatabaseProvider for MongoProvider {
    async fn connect(&mut self, config: &ConnectionConfig) -> DatabaseResult<()> {
        let password = match &config.password {
            Some(p) if !p.is_empty() => p.clone(),
            _ => {
                if config.username.is_empty() {
                    String::new()
                } else {
                    match crate::storage::credentials::get_password(&config.password_key()) {
                        Ok(Some(p)) => p,
                        Ok(None) => {
                            tracing::debug!("No password found in keychain for {}", config.name);
                            String::new()
                        }
                        Err(e) => {
                            tracing::debug!("Keychain error for {}: {}", config.name, e);
                            String::new()
                        }
                    }
                }
            }
        };

        let uri = config
            .connection_string(Some(&password))
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;

        tracing::debug!(
            "Connecting to MongoDB at {}:{}/{} as {}",
            config.host,
            config.port,
            config.database,
            config.username,
        );

        let client = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            Client::with_uri_str(&uri),
        )
        .await
        .map_err(|_| DatabaseError::Connection("Connection timed out".to_string()))?
        .map_err(|e| DatabaseError::Connection(e.to_string()))?;

        let database_name = if config.database.is_empty() {
            "admin".to_string()
        } else {
            config.database.clone()
        };

        // Verify the connection is real — Client::with_uri_str is lazy
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client
                .database(&database_name)
                .run_command(bson::doc! { "ping": 1 }),
        )
        .await
        .map_err(|_| {
            DatabaseError::Connection(
                "Connection timed out — no MongoDB server is reachable".to_string(),
            )
        })?
        .map_err(|e| DatabaseError::Connection(format!("Server unreachable: {}", e)))?;

        self.client = Some(client);
        self.database_name = database_name;

        Ok(())
    }

    async fn disconnect(&mut self) -> DatabaseResult<()> {
        if let Some(client) = self.client.take() {
            client.shutdown().await;
        }
        Ok(())
    }

    async fn ping(&self) -> DatabaseResult<bool> {
        let client = self.client()?;
        client
            .database(&self.database_name)
            .run_command(bson::doc! { "ping": 1 })
            .await
            .map(|_| true)
            .map_err(|e| DatabaseError::Connection(e.to_string()))
    }

    async fn execute_query(&self, sql: &str) -> DatabaseResult<QueryResult> {
        let start = std::time::Instant::now();
        let client = self.client()?;

        let json_val: serde_json::Value = serde_json::from_str(sql)
            .map_err(|e| DatabaseError::Execution(format!("Invalid JSON: {}", e)))?;

        let obj = json_val
            .as_object()
            .ok_or_else(|| DatabaseError::Execution("Query must be a JSON object".to_string()))?;

        let db_name = obj
            .get("db")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.database_name);
        let db = client.database(db_name);

        let recognized_cmds = [
            "find",
            "aggregate",
            "insertOne",
            "insertMany",
            "updateOne",
            "updateMany",
            "deleteOne",
            "deleteMany",
            "count",
            "distinct",
            "collStats",
            "explain",
        ];

        let cmd = match recognized_cmds
            .iter()
            .find(|cmd| obj.get::<str>(*cmd).is_some())
        {
            Some(c) => *c,
            None => {
                if let Some((key, val)) = obj.iter().next() {
                    if val.as_i64() == Some(1) || val.as_f64() == Some(1.0) {
                        return self
                            .exec_find(
                                &db,
                                key,
                                &Bson::Document(Document::new()),
                                None,
                                None,
                                start,
                            )
                            .await;
                    }
                }
                return Err(DatabaseError::Execution(
                    "Unrecognized query command. Supported: find, aggregate, insertOne, insertMany, updateOne, updateMany, deleteOne, deleteMany, count, distinct, collStats, explain".to_string(),
                ));
            }
        };

        let collection_name = obj.get(cmd).and_then(|v| v.as_str()).ok_or_else(|| {
            DatabaseError::Execution(format!(
                "Expected string value for \"{}\" key (collection name)",
                cmd
            ))
        })?;

        match cmd {
            "find" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                let limit = obj.get("limit").and_then(|v| v.as_i64());
                let sort = obj.get("sort").map(json_to_doc).transpose()?;
                self.exec_find(
                    &db,
                    collection_name,
                    &Bson::Document(filter),
                    sort,
                    limit,
                    start,
                )
                .await
            }
            "aggregate" => {
                let pipeline_val = obj.get("pipeline").ok_or_else(|| {
                    DatabaseError::Execution("Missing \"pipeline\" for aggregate".to_string())
                })?;
                let pipeline = json_to_docs(pipeline_val)?;
                self.exec_aggregate(&db, collection_name, pipeline, start)
                    .await
            }
            "insertOne" => {
                let doc_val = obj.get("document").ok_or_else(|| {
                    DatabaseError::Execution("Missing \"document\" for insertOne".to_string())
                })?;
                let document = json_to_doc(doc_val)?;
                self.exec_insert_one(&db, collection_name, document, start)
                    .await
            }
            "insertMany" => {
                let docs_val = obj.get("documents").ok_or_else(|| {
                    DatabaseError::Execution("Missing \"documents\" for insertMany".to_string())
                })?;
                let documents = json_to_docs(docs_val)?;
                self.exec_insert_many(&db, collection_name, documents, start)
                    .await
            }
            "updateOne" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                let update = obj.get("update").ok_or_else(|| {
                    DatabaseError::Execution("Missing \"update\" for updateOne".to_string())
                })?;
                let update_doc = json_to_doc(update)?;
                self.exec_update_one(&db, collection_name, filter, update_doc, start)
                    .await
            }
            "updateMany" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                let update = obj.get("update").ok_or_else(|| {
                    DatabaseError::Execution("Missing \"update\" for updateMany".to_string())
                })?;
                let update_doc = json_to_doc(update)?;
                self.exec_update_many(&db, collection_name, filter, update_doc, start)
                    .await
            }
            "deleteOne" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                self.exec_delete_one(&db, collection_name, filter, start)
                    .await
            }
            "deleteMany" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                self.exec_delete_many(&db, collection_name, filter, start)
                    .await
            }
            "count" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                self.exec_count(&db, collection_name, filter, start).await
            }
            "distinct" => {
                let field = obj.get("field").and_then(|v| v.as_str()).ok_or_else(|| {
                    DatabaseError::Execution("Missing \"field\" for distinct".to_string())
                })?;
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                self.exec_distinct(&db, collection_name, field, filter, start)
                    .await
            }
            "collStats" => self.exec_coll_stats(&db, collection_name, start).await,
            "explain" => {
                let filter = obj
                    .get("filter")
                    .map(json_to_doc)
                    .transpose()?
                    .unwrap_or_default();
                self.exec_explain(&db, collection_name, filter, start).await
            }
            _ => unreachable!(),
        }
    }

    async fn get_databases(&self) -> DatabaseResult<Vec<String>> {
        let client = self.client()?;
        client
            .list_database_names()
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))
    }

    async fn get_schemas(&self) -> DatabaseResult<Vec<String>> {
        // MongoDB has no schemas. The caller in connect_to() will fall back
        // to using each db name as its own synthetic schema, which gives
        // flat layout (db → Tables) and correct per-database collection listing.
        Ok(vec![])
    }

    async fn get_tables(&self, schema: &str) -> DatabaseResult<Vec<TableInfo>> {
        let client = self.client()?;
        let db_name = if schema.is_empty() {
            &self.database_name
        } else {
            schema
        };
        let db = client.database(db_name);
        let names = db
            .list_collection_names()
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?;
        Ok(names
            .into_iter()
            .map(|name| TableInfo {
                name,
                schema: self.database_name.clone(),
                table_type: "COLLECTION".to_string(),
            })
            .collect())
    }

    async fn get_columns(&self, table: &str) -> DatabaseResult<Vec<ColumnInfo>> {
        let client = self.client()?;
        let db = client.database(&self.database_name);
        let collection = db.collection::<Document>(table);

        let cursor = collection
            .find(bson::doc! {})
            .limit(100)
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?;

        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?;

        if docs.is_empty() {
            return Ok(vec![ColumnInfo {
                name: "_id".to_string(),
                ordinal: 0,
                type_name: "objectId".to_string(),
                nullable: true,
                is_pk: true,
                is_fk: false,
                default: None,
                is_auto_increment: false,
            }]);
        }

        let mut field_defs: BTreeMap<String, (String, bool)> = BTreeMap::new();
        for doc in &docs {
            collect_fields(doc, "", &mut field_defs);
        }

        Ok(field_defs
            .into_iter()
            .enumerate()
            .map(|(i, (name, (type_name, is_pk)))| ColumnInfo {
                name,
                ordinal: i,
                type_name,
                nullable: true,
                is_pk,
                is_fk: false,
                default: None,
                is_auto_increment: false,
            })
            .collect())
    }

    async fn get_indexes(&self, table: &str) -> DatabaseResult<Vec<IndexInfo>> {
        let client = self.client()?;
        let db = client.database(&self.database_name);
        let collection = db.collection::<Document>(table);
        let mut cursor = collection
            .list_indexes()
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?;

        let mut indexes = Vec::new();
        while let Some(index_model) = cursor
            .try_next()
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?
        {
            let name = index_model
                .options
                .as_ref()
                .and_then(|o| o.name.clone())
                .unwrap_or_default();
            let columns: Vec<String> = index_model.keys.keys().map(|k| k.to_string()).collect();
            let unique = index_model
                .options
                .as_ref()
                .and_then(|o| o.unique)
                .unwrap_or(false);
            let primary = columns.len() == 1 && columns[0] == "_id";

            indexes.push(IndexInfo {
                name,
                columns,
                unique,
                primary,
            });
        }

        Ok(indexes)
    }

    async fn get_views(&self, _: &str) -> DatabaseResult<Vec<ViewInfo>> {
        Ok(Vec::new())
    }

    async fn get_procedures(&self, _: &str) -> DatabaseResult<Vec<ProcedureInfo>> {
        Ok(Vec::new())
    }

    async fn get_sequences(&self, _: &str) -> DatabaseResult<Vec<SequenceInfo>> {
        Ok(Vec::new())
    }

    async fn get_triggers(&self, _: &str) -> DatabaseResult<Vec<TriggerInfo>> {
        Ok(Vec::new())
    }

    async fn get_functions(&self, _: &str) -> DatabaseResult<Vec<FunctionInfo>> {
        Ok(Vec::new())
    }

    async fn get_extensions(&self) -> DatabaseResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn get_custom_types(&self, _: &str) -> DatabaseResult<Vec<TypeSchemaInfo>> {
        Ok(Vec::new())
    }

    async fn begin_transaction(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn commit(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn rollback(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn cancel_query(&self) -> DatabaseResult<()> {
        Ok(())
    }

    async fn server_info(&self) -> DatabaseResult<ServerInfo> {
        let client = self.client()?;
        let db = client.database(&self.database_name);
        let info = db
            .run_command(bson::doc! { "buildInfo": 1 })
            .await
            .map_err(|e| DatabaseError::Metadata(e.to_string()))?;

        let version = info
            .get_str("version")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(ServerInfo {
            version,
            vendor: "MongoDB".to_string(),
            connection_uptime: None,
        })
    }
}

impl MongoProvider {
    async fn exec_find(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: &Bson,
        sort: Option<Document>,
        limit: Option<i64>,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let filter_doc = match filter {
            Bson::Document(d) => d.clone(),
            _ => Document::new(),
        };

        let mut find = coll.find(filter_doc);
        if let Some(s) = sort {
            find = find.sort(s);
        }
        if let Some(l) = limit {
            find = find.limit(l);
        }

        let cursor = find
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;

        if docs.is_empty() {
            return Ok(QueryResult::new_select(
                Vec::new(),
                Vec::new(),
                start.elapsed(),
            ));
        }

        let columns = docs_to_columns(&docs);
        let rows: Vec<Vec<Value>> = docs.iter().map(|d| doc_to_row(d, &columns)).collect();

        let mut result = QueryResult::new_select(columns, rows, start.elapsed());
        result.raw_documents = Some(docs);
        Ok(result)
    }

    async fn exec_aggregate(
        &self,
        db: &mongodb::Database,
        collection: &str,
        pipeline: Vec<Document>,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let cursor = coll
            .aggregate(pipeline)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;

        if docs.is_empty() {
            return Ok(QueryResult::new_select(
                Vec::new(),
                Vec::new(),
                start.elapsed(),
            ));
        }

        let columns = docs_to_columns(&docs);
        let rows: Vec<Vec<Value>> = docs.iter().map(|d| doc_to_row(d, &columns)).collect();

        let mut result = QueryResult::new_select(columns, rows, start.elapsed());
        result.raw_documents = Some(docs);
        Ok(result)
    }

    async fn exec_insert_one(
        &self,
        db: &mongodb::Database,
        collection: &str,
        document: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        coll.insert_one(document)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        Ok(QueryResult::new_execute(1u64, start.elapsed()))
    }

    async fn exec_insert_many(
        &self,
        db: &mongodb::Database,
        collection: &str,
        documents: Vec<Document>,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let result = coll
            .insert_many(documents)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        Ok(QueryResult::new_execute(
            result.inserted_ids.len() as u64,
            start.elapsed(),
        ))
    }

    async fn exec_update_one(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: Document,
        update: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let result = coll
            .update_one(filter, update)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        let affected = result.modified_count;
        Ok(QueryResult::new_execute(affected, start.elapsed()))
    }

    async fn exec_update_many(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: Document,
        update: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let result = coll
            .update_many(filter, update)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        let affected = result.modified_count;
        Ok(QueryResult::new_execute(affected, start.elapsed()))
    }

    async fn exec_delete_one(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let result = coll
            .delete_one(filter)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        Ok(QueryResult::new_execute(
            result.deleted_count,
            start.elapsed(),
        ))
    }

    async fn exec_delete_many(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let result = coll
            .delete_many(filter)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;
        Ok(QueryResult::new_execute(
            result.deleted_count,
            start.elapsed(),
        ))
    }

    async fn exec_count(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let count = coll
            .count_documents(filter)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;

        let columns = vec![ColumnInfo {
            name: "count".to_string(),
            ordinal: 0,
            type_name: "int64".to_string(),
            nullable: false,
            is_pk: false,
            is_fk: false,
            default: None,
            is_auto_increment: false,
        }];
        let rows = vec![vec![Value::Integer(count as i64)]];

        Ok(QueryResult::new_select(columns, rows, start.elapsed()))
    }

    async fn exec_distinct(
        &self,
        db: &mongodb::Database,
        collection: &str,
        field: &str,
        filter: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let coll = db.collection::<Document>(collection);
        let bson_values = coll
            .distinct(field, filter)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;

        let columns = vec![ColumnInfo {
            name: field.to_string(),
            ordinal: 0,
            type_name: "mixed".to_string(),
            nullable: true,
            is_pk: false,
            is_fk: false,
            default: None,
            is_auto_increment: false,
        }];
        let rows: Vec<Vec<Value>> = bson_values.iter().map(|v| vec![bson_to_value(v)]).collect();

        Ok(QueryResult::new_select(columns, rows, start.elapsed()))
    }

    async fn exec_coll_stats(
        &self,
        db: &mongodb::Database,
        collection: &str,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let result = db
            .run_command(bson::doc! { "collStats": collection })
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;

        let docs = vec![result];
        let columns = docs_to_columns(&docs);
        let rows: Vec<Vec<Value>> = docs.iter().map(|d| doc_to_row(d, &columns)).collect();

        let mut result = QueryResult::new_select(columns, rows, start.elapsed());
        result.raw_documents = Some(docs);
        Ok(result)
    }

    async fn exec_explain(
        &self,
        db: &mongodb::Database,
        collection: &str,
        filter: Document,
        start: std::time::Instant,
    ) -> DatabaseResult<QueryResult> {
        let explain_cmd = bson::doc! {
            "explain": {
                "find": collection,
                "filter": Bson::Document(filter),
            }
        };
        let result = db
            .run_command(explain_cmd)
            .await
            .map_err(|e| DatabaseError::Execution(e.to_string()))?;

        let docs = vec![result];
        let columns = docs_to_columns(&docs);
        let rows: Vec<Vec<Value>> = docs.iter().map(|d| doc_to_row(d, &columns)).collect();

        let mut result = QueryResult::new_select(columns, rows, start.elapsed());
        result.raw_documents = Some(docs);
        Ok(result)
    }
}
