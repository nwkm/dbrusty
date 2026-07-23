use std::collections::HashSet;
use std::sync::OnceLock;

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::completion::sql_keywords;

fn keyword_style() -> Style {
    Style::default().fg(Color::Cyan)
}
fn string_style() -> Style {
    Style::default().fg(Color::Yellow)
}
fn number_style() -> Style {
    Style::default().fg(Color::Magenta)
}
fn comment_style() -> Style {
    Style::default().fg(Color::DarkGray)
}
fn operator_style() -> Style {
    Style::default().fg(Color::Red)
}
fn type_style() -> Style {
    Style::default().fg(Color::Green)
}
fn function_style() -> Style {
    Style::default().fg(Color::LightMagenta)
}
fn mongo_operator_style() -> Style {
    Style::default().fg(Color::LightBlue)
}
fn redis_command_style() -> Style {
    Style::default().fg(Color::LightRed)
}

fn sql_keywords_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| sql_keywords::KEYWORDS.iter().copied().collect())
}

fn sql_types_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| sql_keywords::TYPES.iter().copied().collect())
}

fn sql_functions_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut s: HashSet<&str> = sql_keywords::FUNCTIONS.iter().copied().collect();
        s.extend(sql_keywords::POSTGRES_FUNCTIONS.iter().copied());
        s.extend(sql_keywords::MYSQL_FUNCTIONS.iter().copied());
        s
    })
}

fn mongo_stages() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "$match",
            "$group",
            "$sort",
            "$project",
            "$limit",
            "$skip",
            "$unwind",
            "$lookup",
            "$addFields",
            "$bucket",
            "$count",
            "$facet",
            "$geoNear",
            "$graphLookup",
            "$out",
            "$merge",
            "$redact",
            "$replaceRoot",
            "$sample",
            "$set",
            "$unset",
            "$replaceWith",
            "$setWindowFields",
            "$densify",
            "$fill",
            "$documents",
            "$unionWith",
            "$collStats",
            "$currentOp",
            "$listLocalSessions",
            "$listSessions",
            "$planCacheStats",
            "$indexStats",
        ]
        .into_iter()
        .collect()
    })
}

fn mongo_operators() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "$eq",
            "$ne",
            "$gt",
            "$gte",
            "$lt",
            "$lte",
            "$in",
            "$nin",
            "$and",
            "$or",
            "$not",
            "$nor",
            "$exists",
            "$type",
            "$regex",
            "$text",
            "$where",
            "$expr",
            "$mod",
            "$all",
            "$elemMatch",
            "$size",
            "$bitsAllClear",
            "$bitsAllSet",
            "$bitsAnyClear",
            "$bitsAnySet",
            "$comment",
            "$natural",
            "$maxTimeMS",
            "$max",
            "$min",
            "$returnKey",
            "$showRecordId",
            "$hint",
            "$slice",
            "$meta",
            "$search",
            "$searchHighlight",
            "$rankFusion",
            "$scored",
            "$near",
            "$nearSphere",
            "$geoWithin",
            "$geoIntersects",
            "$geoNear",
            "$center",
            "$centerSphere",
            "$box",
            "$polygon",
            "$uniqueDocs",
            "$inc",
            "$mul",
            "$rename",
            "$setOnInsert",
            "$unset",
            "$min",
            "$max",
            "$currentDate",
            "$push",
            "$addToSet",
            "$pop",
            "$pull",
            "$pullAll",
            "$each",
            "$position",
            "$sort",
            "$slice",
        ]
        .into_iter()
        .collect()
    })
}

fn redis_commands() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "GET",
            "SET",
            "DEL",
            "EXISTS",
            "EXPIRE",
            "TTL",
            "KEYS",
            "SCAN",
            "TYPE",
            "RENAME",
            "FLUSHALL",
            "FLUSHDB",
            "HSET",
            "HGET",
            "HDEL",
            "HGETALL",
            "HKEYS",
            "HVALS",
            "HLEN",
            "LPUSH",
            "RPUSH",
            "LPOP",
            "RPOP",
            "LLEN",
            "LRANGE",
            "LINDEX",
            "SADD",
            "SREM",
            "SMEMBERS",
            "SISMEMBER",
            "SCARD",
            "SINTER",
            "ZADD",
            "ZREM",
            "ZRANGE",
            "ZREVRANGE",
            "ZCARD",
            "ZSCORE",
            "PUBLISH",
            "SUBSCRIBE",
            "PSUBSCRIBE",
            "PUBSUB",
            "MULTI",
            "EXEC",
            "DISCARD",
            "WATCH",
            "UNWATCH",
            "INFO",
            "CLIENT",
            "CONFIG",
            "SLOWLOG",
            "MONITOR",
            "SELECT",
            "MOVE",
            "RESTORE",
            "DUMP",
            "OBJECT",
            "EVAL",
            "EVALSHA",
            "SCRIPT",
            "SORT",
            "TOUCH",
            "GEOADD",
            "GEODIST",
            "GEOHASH",
            "GEOPOS",
            "GEORADIUS",
            "PFADD",
            "PFCOUNT",
            "PFMERGE",
            "BITCOUNT",
            "BITOP",
            "BITFIELD",
            "GETBIT",
            "SETBIT",
            "ACL",
            "AUTH",
            "HELLO",
            "PING",
            "ECHO",
            "QUIT",
            "SAVE",
            "BGSAVE",
            "BGREWRITEAOF",
            "LASTSAVE",
            "SHUTDOWN",
            "SLAVEOF",
            "REPLICAOF",
            "ROLE",
            "TIME",
            "WAIT",
            "SWAPDB",
            "MIGRATE",
            "XADD",
            "XREAD",
            "XRANGE",
            "XREVRANGE",
            "XDEL",
            "XTRIM",
            "XLEN",
            "XGROUP",
            "XREADGROUP",
            "XACK",
            "XCLAIM",
            "XPENDING",
            "STRLEN",
            "APPEND",
            "GETRANGE",
            "SETRANGE",
            "INCR",
            "INCRBY",
            "INCRBYFLOAT",
            "DECR",
            "DECRBY",
            "MGET",
            "MSET",
            "MSETNX",
            "GETSET",
            "GETEX",
            "GETDEL",
            "COPY",
            "SUBSTR",
        ]
        .into_iter()
        .collect()
    })
}

pub fn highlight_sql_line<'a>(line: &str) -> Vec<Span<'a>> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut spans = Vec::new();

    while i < len {
        if i + 1 < len && chars[i] == '-' && chars[i + 1] == '-' {
            spans.push(Span::styled(
                chars[i..].iter().collect::<String>(),
                comment_style(),
            ));
            break;
        }

        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            let mut end = i + 2;
            while end + 1 < len && !(chars[end] == '*' && chars[end + 1] == '/') {
                end += 1;
            }
            end = (end + 2).min(len);
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                comment_style(),
            ));
            i = end;
            continue;
        }

        if chars[i] == '\'' {
            let mut end = i + 1;
            while end < len && chars[end] != '\'' {
                if chars[end] == '\\' {
                    end += 1;
                }
                end += 1;
            }
            if end < len {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                string_style(),
            ));
            i = end;
            continue;
        }

        if chars[i] == '"' {
            let mut end = i + 1;
            while end < len && chars[end] != '"' {
                end += 1;
            }
            if end < len {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                string_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_ascii_digit()
            || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let mut end = i + 1;
            while end < len && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                number_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut end = i + 1;
            while end < len && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            let upper = word.to_uppercase();
            if sql_functions_set().contains(upper.as_str()) {
                spans.push(Span::styled(word, function_style()));
            } else if sql_types_set().contains(upper.as_str()) {
                spans.push(Span::styled(word, type_style()));
            } else if sql_keywords_set().contains(upper.as_str()) {
                spans.push(Span::styled(word, keyword_style()));
            } else {
                spans.push(Span::raw(word));
            }
            i = end;
            continue;
        }

        if chars[i] == ';' {
            spans.push(Span::styled(";", operator_style()));
            i += 1;
            continue;
        }

        if "+-*/%=<>!&|^~".contains(chars[i]) {
            let mut end = i + 1;
            while end < len && "+-*/%=<>!&|^~".contains(chars[end]) {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                operator_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_whitespace() {
            let mut end = i + 1;
            while end < len && chars[end].is_whitespace() {
                end += 1;
            }
            spans.push(Span::raw(chars[i..end].iter().collect::<String>()));
            i = end;
        } else {
            spans.push(Span::raw(chars[i].to_string()));
            i += 1;
        }
    }
    spans
}

pub fn highlight_mongo_line<'a>(line: &str) -> Vec<Span<'a>> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut spans = Vec::new();

    while i < len {
        if chars[i] == '$' && i + 1 < len && chars[i + 1].is_alphabetic() {
            let mut end = i + 1;
            while end < len && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            if mongo_stages().contains(word.as_str()) || mongo_operators().contains(word.as_str()) {
                spans.push(Span::styled(word, mongo_operator_style()));
            } else {
                spans.push(Span::raw(word));
            }
            i = end;
            continue;
        }

        if chars[i] == '\'' {
            let mut end = i + 1;
            while end < len && chars[end] != '\'' {
                end += 1;
            }
            if end < len {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                string_style(),
            ));
            i = end;
            continue;
        }

        if chars[i] == '"' {
            let mut end = i + 1;
            while end < len && chars[end] != '"' {
                end += 1;
            }
            if end < len {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                string_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_ascii_digit() {
            let mut end = i + 1;
            while end < len && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                number_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut end = i + 1;
            while end < len && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            let upper = word.to_uppercase();
            if sql_keywords_set().contains(upper.as_str()) {
                spans.push(Span::styled(word, keyword_style()));
            } else {
                spans.push(Span::raw(word));
            }
            i = end;
            continue;
        }

        if "{}[]".contains(chars[i]) {
            spans.push(Span::styled(
                chars[i].to_string(),
                Style::default().fg(Color::Gray),
            ));
            i += 1;
            continue;
        }

        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            spans.push(Span::styled(
                chars[i..].iter().collect::<String>(),
                comment_style(),
            ));
            break;
        }

        if chars[i].is_whitespace() {
            let mut end = i + 1;
            while end < len && chars[end].is_whitespace() {
                end += 1;
            }
            spans.push(Span::raw(chars[i..end].iter().collect::<String>()));
            i = end;
        } else {
            spans.push(Span::raw(chars[i].to_string()));
            i += 1;
        }
    }
    spans
}

pub fn highlight_redis_line<'a>(line: &str) -> Vec<Span<'a>> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut spans = Vec::new();

    while i < len {
        if chars[i] == '\'' {
            let mut end = i + 1;
            while end < len && chars[end] != '\'' {
                end += 1;
            }
            if end < len {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                string_style(),
            ));
            i = end;
            continue;
        }

        if chars[i] == '"' {
            let mut end = i + 1;
            while end < len && chars[end] != '"' {
                end += 1;
            }
            if end < len {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                string_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_ascii_digit() {
            let mut end = i + 1;
            while end < len && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            spans.push(Span::styled(
                chars[i..end].iter().collect::<String>(),
                number_style(),
            ));
            i = end;
            continue;
        }

        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut end = i + 1;
            while end < len && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let word: String = chars[i..end].iter().collect();
            let upper = word.to_uppercase();
            if redis_commands().contains(upper.as_str()) {
                spans.push(Span::styled(word, redis_command_style()));
            } else {
                spans.push(Span::raw(word));
            }
            i = end;
            continue;
        }

        if chars[i] == ';' {
            spans.push(Span::styled(";", operator_style()));
            i += 1;
            continue;
        }

        if chars[i].is_whitespace() {
            let mut end = i + 1;
            while end < len && chars[end].is_whitespace() {
                end += 1;
            }
            spans.push(Span::raw(chars[i..end].iter().collect::<String>()));
            i = end;
        } else {
            spans.push(Span::raw(chars[i].to_string()));
            i += 1;
        }
    }
    spans
}
