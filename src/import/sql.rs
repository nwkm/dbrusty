use std::fs;

use super::ImportPreview;

pub fn import_sql(path: &str) -> Result<ImportPreview, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;

    let statements = split_sql(&content);

    if statements.is_empty() {
        return Err("No SQL statements found in file".to_string());
    }

    Ok(ImportPreview {
        estimated_rows: 0,
        columns: Vec::new(),
        sample_rows: Vec::new(),
        sql_statements: statements,
    })
}

fn split_sql(content: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_char = '\'';
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if c == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if !in_string {
            if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                in_block_comment = true;
                i += 2;
                continue;
            }
        }

        if c == '\'' || c == '"' {
            if in_string {
                if c == string_char {
                    in_string = false;
                }
            } else {
                in_string = true;
                string_char = c;
            }
        }

        if !in_string && c == ';' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                statements.push(trimmed);
            }
            current.clear();
        } else {
            current.push(c);
        }

        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}
