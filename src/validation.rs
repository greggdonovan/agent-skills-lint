//! Metadata validation for SKILL.md frontmatter.
//!
//! This module contains all validation logic for skill metadata fields,
//! including name format rules, length limits, and directory matching.
//!
//! # Validation Rules
//!
//! - `name`: Required, lowercase, 1-64 chars, alphanumeric + hyphen only,
//!   no leading/trailing/consecutive hyphens, must match directory name (NFKC normalized)
//! - `description`: Required, 1-1024 chars
//! - `license`: Optional string, must be non-empty if provided
//! - `compatibility`: Optional, max 500 chars
//! - `allowed-tools`: Optional, string or array of tool specifications
//! - `metadata`: Optional key-value pairs

use std::collections::BTreeMap;
use std::path::Path;

use serde_yaml::Value;
use unicode_normalization::UnicodeNormalization;

use crate::skill::{
    ALLOWED_FIELDS, MAX_COMPATIBILITY_LENGTH, MAX_DESCRIPTION_LENGTH, MAX_SKILL_NAME_LENGTH,
};

/// Validate the metadata extracted from a SKILL.md file.
///
/// Returns a list of validation errors. An empty list indicates the metadata is valid.
///
/// # Arguments
///
/// * `metadata` - The parsed frontmatter metadata.
/// * `skill_dir` - Optional path to the skill directory for name matching validation.
pub fn validate_metadata(
    metadata: &BTreeMap<String, Value>,
    skill_dir: Option<&Path>,
) -> Vec<String> {
    let mut errors = Vec::new();

    let extra_fields: Vec<String> = metadata
        .keys()
        .filter(|key| !ALLOWED_FIELDS.contains(&key.as_str()))
        .cloned()
        .collect();
    if !extra_fields.is_empty() {
        errors.push(format!(
            "Unexpected fields in frontmatter: {}. Only {:?} are allowed.",
            extra_fields.join(", "),
            ALLOWED_FIELDS
        ));
    }

    match metadata.get("name") {
        Some(Value::String(name)) if !name.trim().is_empty() => {
            errors.extend(validate_name(name, skill_dir));
        }
        Some(_) => errors.push("Field 'name' must be a non-empty string".to_string()),
        None => errors.push("Missing required field in frontmatter: name".to_string()),
    }

    match metadata.get("description") {
        Some(Value::String(description)) if !description.trim().is_empty() => {
            errors.extend(validate_description(description));
        }
        Some(_) => errors.push("Field 'description' must be a non-empty string".to_string()),
        None => errors.push("Missing required field in frontmatter: description".to_string()),
    }

    if let Some(value) = metadata.get("license") {
        errors.extend(validate_license(value));
    }

    if let Some(value) = metadata.get("compatibility") {
        match value {
            Value::String(text) => errors.extend(validate_compatibility(text)),
            _ => errors.push("Field 'compatibility' must be a string".to_string()),
        }
    }

    if let Some(value) = metadata.get("allowed-tools") {
        errors.extend(validate_allowed_tools(value));
    }

    errors
}

fn validate_name(name: &str, skill_dir: Option<&Path>) -> Vec<String> {
    let mut errors = Vec::new();

    if name.trim().is_empty() {
        errors.push("Field 'name' must be a non-empty string".to_string());
        return errors;
    }

    let normalized: String = name.trim().nfkc().collect();
    let char_count = normalized.chars().count();

    if char_count > MAX_SKILL_NAME_LENGTH {
        errors.push(format!(
            "Skill name '{normalized}' exceeds {MAX_SKILL_NAME_LENGTH} character limit ({char_count} chars)"
        ));
    }

    if normalized != normalized.to_lowercase() {
        errors.push(format!("Skill name '{normalized}' must be lowercase"));
    }

    if normalized.starts_with('-') || normalized.ends_with('-') {
        errors.push("Skill name cannot start or end with a hyphen".to_string());
    }

    if normalized.contains("--") {
        errors.push("Skill name cannot contain consecutive hyphens".to_string());
    }

    if !normalized.chars().all(|c| c.is_alphanumeric() || c == '-') {
        errors.push(format!(
            "Skill name '{normalized}' contains invalid characters. Only letters, digits, and hyphens are allowed."
        ));
    }

    if let Some(dir) = skill_dir {
        if let Some(dir_name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) {
            let dir_norm: String = dir_name.nfkc().collect();
            if dir_norm != normalized {
                errors.push(format!(
                    "Directory name '{dir_name}' must match skill name '{normalized}'"
                ));
            }
        }
    }

    errors
}

fn validate_description(description: &str) -> Vec<String> {
    let mut errors = Vec::new();

    if description.trim().is_empty() {
        errors.push("Field 'description' must be a non-empty string".to_string());
        return errors;
    }

    let char_count = description.chars().count();
    if char_count > MAX_DESCRIPTION_LENGTH {
        errors.push(format!(
            "Description exceeds {MAX_DESCRIPTION_LENGTH} character limit ({char_count} chars)"
        ));
    }

    errors
}

fn validate_compatibility(compatibility: &str) -> Vec<String> {
    let mut errors = Vec::new();

    let char_count = compatibility.chars().count();
    if char_count > MAX_COMPATIBILITY_LENGTH {
        errors.push(format!(
            "Compatibility exceeds {MAX_COMPATIBILITY_LENGTH} character limit ({char_count} chars)"
        ));
    }

    errors
}

fn validate_license(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    match value {
        Value::String(license) => {
            if license.trim().is_empty() {
                errors.push("Field 'license' must be a non-empty string if provided".to_string());
            }
        }
        _ => {
            errors.push("Field 'license' must be a string".to_string());
        }
    }

    errors
}

fn validate_allowed_tools(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    match value {
        Value::String(tools) => {
            if tools.trim().is_empty() {
                errors.push(
                    "Field 'allowed-tools' must be a non-empty string if provided".to_string(),
                );
            }
            // Validate the format: should be comma-separated tool specifications
            // Each tool spec is: ToolName or ToolName(pattern) or ToolName(pattern:subpattern)
            for tool in tools.split(',') {
                let tool = tool.trim();
                if tool.is_empty() {
                    continue;
                }
                // Basic format validation: should start with an uppercase letter (tool name)
                if let Some(first_char) = tool.chars().next() {
                    if !first_char.is_ascii_uppercase() && first_char != '*' {
                        errors.push(format!(
                            "Invalid tool specification '{tool}': tool names should start with an uppercase letter or be '*'"
                        ));
                    }
                }
                // Check for balanced parentheses
                let open_parens = tool.chars().filter(|&c| c == '(').count();
                let close_parens = tool.chars().filter(|&c| c == ')').count();
                if open_parens != close_parens {
                    errors.push(format!(
                        "Invalid tool specification '{tool}': unbalanced parentheses"
                    ));
                }
            }
        }
        Value::Sequence(seq) => {
            // Also accept an array of tool strings
            for (i, item) in seq.iter().enumerate() {
                if !matches!(item, Value::String(_)) {
                    errors.push(format!(
                        "Field 'allowed-tools' array item {i} must be a string"
                    ));
                }
            }
        }
        _ => {
            errors.push("Field 'allowed-tools' must be a string or array of strings".to_string());
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_metadata() -> BTreeMap<String, Value> {
        let mut metadata = BTreeMap::new();
        metadata.insert("name".to_string(), Value::String("my-skill".to_string()));
        metadata.insert(
            "description".to_string(),
            Value::String("A test skill".to_string()),
        );
        metadata
    }

    #[test]
    fn test_validate_license_valid() {
        let mut metadata = base_metadata();
        metadata.insert("license".to_string(), Value::String("MIT".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_license_empty() {
        let mut metadata = base_metadata();
        metadata.insert("license".to_string(), Value::String(String::new()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|e| e.contains("license")));
    }

    #[test]
    fn test_validate_license_wrong_type() {
        let mut metadata = base_metadata();
        metadata.insert("license".to_string(), Value::Number(123.into()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|e| e.contains("license")));
    }

    #[test]
    fn test_validate_allowed_tools_valid() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(git:*)".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_allowed_tools_multiple() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(git:*), Read, Write".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_allowed_tools_wildcard() {
        let mut metadata = base_metadata();
        metadata.insert("allowed-tools".to_string(), Value::String("*".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_allowed_tools_unbalanced_parens() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(git:*".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|e| e.contains("unbalanced")));
    }

    #[test]
    fn test_validate_allowed_tools_invalid_name() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("bash".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|e| e.contains("uppercase")));
    }

    #[test]
    fn test_validate_allowed_tools_array() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::Sequence(vec![
                Value::String("Bash".to_string()),
                Value::String("Read".to_string()),
            ]),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());
    }
}
