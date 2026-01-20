//! Frontmatter formatting utilities.
//!
//! This module handles parsing and formatting of YAML frontmatter in SKILL.md files.
//! It provides deterministic output with consistent field ordering and proper quoting.

use std::collections::BTreeMap;

use serde_yaml::{Mapping, Value};

use crate::error::ParseError;
use crate::skill::FIELD_ORDER;

/// Parse YAML frontmatter from file content.
///
/// Extracts the YAML frontmatter between `---` delimiters and returns the
/// parsed metadata along with the body content.
///
/// # Arguments
///
/// * `content` - The full file content, optionally with a UTF-8 BOM.
///
/// # Returns
///
/// A tuple of (metadata, body) where metadata is a map of field names to values
/// and body is the content after the closing `---`.
///
/// # Errors
///
/// Returns an error if:
/// - The content doesn't start with `---`
/// - The frontmatter isn't properly closed
/// - The YAML is invalid
/// - The frontmatter isn't a mapping
/// - Any key isn't a string
pub fn parse_frontmatter(content: &str) -> Result<(BTreeMap<String, Value>, String), ParseError> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    if !content.starts_with("---") {
        return Err(ParseError::MissingFrontmatter);
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(ParseError::UnclosedFrontmatter);
    }

    let frontmatter_str = parts[1];
    let body = parts[2].trim().to_string();

    let parsed: Value = serde_yaml::from_str(frontmatter_str)?;

    match parsed {
        Value::Mapping(map) => Ok((mapping_to_btreemap(map)?, body)),
        _ => Err(ParseError::NotAMapping),
    }
}

/// Format metadata back into YAML frontmatter.
///
/// Produces a deterministic output with fields in a specific order and
/// proper quoting for values that need it.
///
/// # Arguments
///
/// * `metadata` - The metadata to format.
///
/// # Returns
///
/// The formatted frontmatter string including the `---` delimiters.
pub fn format_frontmatter(metadata: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut lines: Vec<String> = vec!["---".to_string()];

    for field in FIELD_ORDER {
        let Some(value) = metadata.get(field) else {
            continue;
        };

        if field == "metadata" {
            if let Value::Mapping(map) = value {
                if map.is_empty() {
                    continue;
                }
                let normalized = mapping_to_string_map(map)?;
                lines.push("metadata:".to_string());
                for (key, val) in normalized {
                    lines.push(format!("  {}: {}", format_key(&key), format_scalar(&val)));
                }
            } else {
                let scalar = value_to_string(value)?;
                lines.push(format!("metadata: {}", format_scalar(&scalar)));
            }
            continue;
        }

        let scalar = value_to_string(value)?;
        lines.push(format!("{}: {}", field, format_scalar(&scalar)));
    }

    let unknown_fields: Vec<String> = metadata
        .keys()
        .filter(|key| !crate::skill::ALLOWED_FIELDS.contains(&key.as_str()))
        .cloned()
        .collect();

    for key in unknown_fields {
        let value = metadata.get(&key).expect("key exists");
        if let Value::Mapping(map) = value {
            let normalized = mapping_to_string_map(map)?;
            lines.push(format!("{}:", format_key(&key)));
            for (sub_key, sub_val) in normalized {
                lines.push(format!(
                    "  {}: {}",
                    format_key(&sub_key),
                    format_scalar(&sub_val)
                ));
            }
        } else {
            let scalar = value_to_string(value)?;
            lines.push(format!("{}: {}", format_key(&key), format_scalar(&scalar)));
        }
    }

    lines.push("---".to_string());
    Ok(lines.join("\n"))
}

/// Convert a YAML Value to a string representation.
pub fn value_to_string(value: &Value) -> Result<String, String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(num) => Ok(num.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Ok("null".to_string()),
        _ => Err("Unsupported YAML value type for formatting".to_string()),
    }
}

/// Convert a `serde_yaml` Mapping to a `BTreeMap` with string keys.
pub fn mapping_to_btreemap(map: Mapping) -> Result<BTreeMap<String, Value>, ParseError> {
    let mut result = BTreeMap::new();
    for (key, value) in map {
        let key_str = match key {
            Value::String(text) => text,
            _ => return Err(ParseError::NonStringKey),
        };
        result.insert(key_str, value);
    }
    Ok(result)
}

/// Convert a Mapping to a `BTreeMap` with string keys and values.
pub fn mapping_to_string_map(map: &Mapping) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for (key, value) in map {
        let key_str = value_to_string(key)?;
        let value_str = value_to_string(value)?;
        result.insert(key_str, value_str);
    }
    Ok(result)
}

/// Format a scalar value, adding quotes if needed.
pub fn format_scalar(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.starts_with(' ')
        || value.starts_with('\t')
        || value.ends_with(' ')
        || value.ends_with('\t')
        || value.contains('\n')
        || value.contains('\r')
        || value.contains('\t')
        || value.contains(':')
        || value.contains('#')
        || value
            .chars()
            .next()
            .map(|first| {
                matches!(
                    first,
                    '-' | '?' | '!' | '@' | '&' | '*' | '>' | '|' | '{' | '}' | '[' | ']' | ','
                )
            })
            .unwrap_or(false);

    if needs_quotes {
        json_quote(value)
    } else {
        value.to_string()
    }
}

/// Format a key, adding quotes if needed.
pub fn format_key(value: &str) -> String {
    if is_simple_key(value) {
        value.to_string()
    } else {
        json_quote(value)
    }
}

fn is_simple_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Derive a description from the body content.
///
/// Looks for the first non-empty, non-heading, non-code-block line.
/// Falls back to the first heading text, or a placeholder if nothing found.
pub fn derive_description(body: &str) -> String {
    let mut in_code_block = false;
    for raw in body.lines() {
        let line = raw.trim();
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        return line.to_string();
    }

    for raw in body.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            return line.trim_start_matches('#').trim().to_string();
        }
    }

    "Describe what this skill does and when to use it".to_string()
}
