use crate::completion::provider::CompletionProvider;
use crate::completion::{CompletionItem, CompletionKind};
use crate::database::metadata::MetadataCache;
use crate::models::connection::DatabaseType;

pub struct MongoCompletionProvider;

const MONGO_OPERATORS: &[&str] = &[
    "$eq",
    "$gt",
    "$gte",
    "$lt",
    "$lte",
    "$ne",
    "$in",
    "$nin",
    "$and",
    "$or",
    "$not",
    "$nor",
    "$exists",
    "$regex",
    "$options",
    "$elemMatch",
    "$size",
    "$type",
    "$all",
    "$mod",
    "$text",
    "$search",
    "$language",
    "$caseSensitive",
    "$diacriticSensitive",
    "$expr",
    "$jsonSchema",
    "$geoWithin",
    "$geoIntersects",
    "$near",
    "$nearSphere",
    "$maxDistance",
    "$minDistance",
    "$box",
    "$polygon",
    "$center",
    "$centerSphere",
    "$uniqueDocs",
    "$slice",
    "$meta",
];

const MONGO_AGGREGATION: &[&str] = &[
    "$match",
    "$group",
    "$sort",
    "$project",
    "$limit",
    "$skip",
    "$unwind",
    "$lookup",
    "$addFields",
    "$replaceRoot",
    "$replaceWith",
    "$count",
    "$facet",
    "$bucket",
    "$bucketAuto",
    "$sample",
    "$set",
    "$unset",
    "$redact",
    "$out",
    "$merge",
    "$sortByCount",
    "$graphLookup",
    "$geoNear",
    "$indexStats",
    "$planCacheStats",
    "$currentOp",
    "$listLocalSessions",
    "$listSessions",
];

const MONGO_ACCUMULATORS: &[&str] = &[
    "$sum",
    "$avg",
    "$first",
    "$last",
    "$max",
    "$min",
    "$push",
    "$addToSet",
    "$stdDevPop",
    "$stdDevSamp",
];

const MONGO_METHODS: &[&str] = &[
    "find",
    "findOne",
    "findOneAndUpdate",
    "findOneAndDelete",
    "findOneAndReplace",
    "insertOne",
    "insertMany",
    "updateOne",
    "updateMany",
    "replaceOne",
    "deleteOne",
    "deleteMany",
    "bulkWrite",
    "aggregate",
    "countDocuments",
    "estimatedDocumentCount",
    "distinct",
    "createIndex",
    "createIndexes",
    "dropIndex",
    "dropIndexes",
    "getIndexes",
    "listIndexes",
    "watch",
    "explain",
    "mapReduce",
    "renameCollection",
    "drop",
    "cloneCollection",
    "convertToCapped",
];

const MONGO_BSON_TYPES: &[&str] = &[
    "ObjectId",
    "ISODate",
    "Date",
    "NumberInt",
    "NumberLong",
    "NumberDecimal",
    "BinData",
    "HexData",
    "UUID",
    "Timestamp",
    "RegExp",
    "DBRef",
    "MinKey",
    "MaxKey",
];

impl CompletionProvider for MongoCompletionProvider {
    fn completions(
        &self,
        _metadata: &MetadataCache,
        _db_type: Option<&DatabaseType>,
    ) -> Vec<CompletionItem> {
        let mut items: Vec<CompletionItem> = Vec::new();

        for op in MONGO_OPERATORS {
            items.push(CompletionItem::new(op, CompletionKind::Operator));
        }

        for op in MONGO_AGGREGATION {
            items.push(CompletionItem::new(op, CompletionKind::Operator));
        }

        for op in MONGO_ACCUMULATORS {
            items.push(CompletionItem::new(op, CompletionKind::Operator));
        }

        for m in MONGO_METHODS {
            items.push(CompletionItem::new(m, CompletionKind::Function));
        }

        for t in MONGO_BSON_TYPES {
            items.push(CompletionItem::new(t, CompletionKind::Type));
        }

        items
    }
}
