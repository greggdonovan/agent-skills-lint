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
//! - `allowed-tools`: Optional, space-delimited string (experimental)
//! - `metadata`: Optional key-value pairs

use std::collections::BTreeMap;
use std::path::Path;

use serde_yaml::Value;
use unicode_normalization::UnicodeNormalization;

use crate::error::ValidationError;
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
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let extra_fields: Vec<String> = metadata
        .keys()
        .filter(|key| !ALLOWED_FIELDS.contains(&key.as_str()))
        .cloned()
        .collect();
    if !extra_fields.is_empty() {
        errors.push(ValidationError::UnexpectedFields {
            fields: extra_fields.join(", "),
            allowed: &ALLOWED_FIELDS,
        });
    }

    match metadata.get("name") {
        Some(Value::String(name)) if !name.trim().is_empty() => {
            errors.extend(validate_name(name, skill_dir));
        }
        Some(_) => errors.push(ValidationError::EmptyField("name".to_string())),
        None => errors.push(ValidationError::MissingField("name".to_string())),
    }

    match metadata.get("description") {
        Some(Value::String(description)) if !description.trim().is_empty() => {
            errors.extend(validate_description(description));
        }
        Some(_) => errors.push(ValidationError::EmptyField("description".to_string())),
        None => errors.push(ValidationError::MissingField("description".to_string())),
    }

    if let Some(value) = metadata.get("license") {
        errors.extend(validate_license(value));
    }

    if let Some(value) = metadata.get("compatibility") {
        match value {
            Value::String(text) => errors.extend(validate_compatibility(text)),
            _ => errors.push(ValidationError::InvalidType("compatibility".to_string())),
        }
    }

    if let Some(value) = metadata.get("metadata") {
        errors.extend(validate_metadata_field(value));
    }

    if let Some(value) = metadata.get("allowed-tools") {
        errors.extend(validate_allowed_tools(value));
    }

    errors
}

fn validate_name(name: &str, skill_dir: Option<&Path>) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if name.trim().is_empty() {
        errors.push(ValidationError::EmptyField("name".to_string()));
        return errors;
    }

    let normalized: String = name.trim().nfkc().collect();
    let char_count = normalized.chars().count();

    if char_count > MAX_SKILL_NAME_LENGTH {
        errors.push(ValidationError::NameTooLong {
            name: normalized.clone(),
            limit: MAX_SKILL_NAME_LENGTH,
            actual: char_count,
        });
    }

    if normalized != normalized.to_lowercase() {
        errors.push(ValidationError::NameNotLowercase(normalized.clone()));
    }

    if normalized.starts_with('-') || normalized.ends_with('-') {
        errors.push(ValidationError::NameInvalidHyphen);
    }

    if normalized.contains("--") {
        errors.push(ValidationError::NameConsecutiveHyphens);
    }

    if !normalized.chars().all(|c| c.is_alphanumeric() || c == '-') {
        errors.push(ValidationError::NameInvalidChars(normalized.clone()));
    }

    if let Some(dir) = skill_dir {
        if let Some(dir_name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) {
            let dir_norm: String = dir_name.nfkc().collect();
            if dir_norm != normalized {
                errors.push(ValidationError::NameMismatch {
                    dir: dir_name,
                    name: normalized,
                });
            }
        }
    }

    errors
}

fn validate_description(description: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if description.trim().is_empty() {
        errors.push(ValidationError::EmptyField("description".to_string()));
        return errors;
    }

    let char_count = description.chars().count();
    if char_count > MAX_DESCRIPTION_LENGTH {
        errors.push(ValidationError::DescriptionTooLong {
            limit: MAX_DESCRIPTION_LENGTH,
            actual: char_count,
        });
    }

    errors
}

fn validate_compatibility(compatibility: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if compatibility.trim().is_empty() {
        errors.push(ValidationError::EmptyField("compatibility".to_string()));
        return errors;
    }

    let char_count = compatibility.chars().count();
    if char_count > MAX_COMPATIBILITY_LENGTH {
        errors.push(ValidationError::CompatibilityTooLong {
            limit: MAX_COMPATIBILITY_LENGTH,
            actual: char_count,
        });
    }

    errors
}

fn validate_metadata_field(value: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let Value::Mapping(map) = value else {
        errors.push(ValidationError::MetadataNotMapping);
        return errors;
    };

    for (key, val) in map {
        let key_str = if let Value::String(text) = key {
            text
        } else {
            errors.push(ValidationError::MetadataNonStringKey);
            continue;
        };
        if !matches!(val, Value::String(_)) {
            errors.push(ValidationError::MetadataNonStringValue {
                key: key_str.clone(),
            });
        }
    }

    errors
}

fn validate_license(value: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match value {
        Value::String(license) => {
            if license.trim().is_empty() {
                errors.push(ValidationError::EmptyLicense);
            }
        }
        _ => {
            errors.push(ValidationError::InvalidType("license".to_string()));
        }
    }

    errors
}

fn validate_allowed_tools(value: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match value {
        Value::String(tools) => {
            if tools.trim().is_empty() {
                errors.push(ValidationError::EmptyField("allowed-tools".to_string()));
            }
            if tools.contains(',') {
                errors.push(ValidationError::InvalidToolSpec {
                    spec: tools.clone(),
                    reason: "tools must be space-delimited, not comma-delimited".to_string(),
                });
            }

            for tool in tools.split_whitespace() {
                if let Some(reason) = validate_tool_spec(tool) {
                    errors.push(ValidationError::InvalidToolSpec {
                        spec: tool.to_string(),
                        reason,
                    });
                }
            }
        }
        Value::Sequence(seq) => {
            // Accept arrays for backward compatibility.
            for (i, item) in seq.iter().enumerate() {
                if !matches!(item, Value::String(_)) {
                    errors.push(ValidationError::InvalidToolArrayItem { index: i });
                }
            }
        }
        _ => {
            errors.push(ValidationError::InvalidToolsType);
        }
    }

    errors
}

fn validate_tool_spec(tool: &str) -> Option<String> {
    let open_parens = tool.chars().filter(|&c| c == '(').count();
    let close_parens = tool.chars().filter(|&c| c == ')').count();
    if open_parens != close_parens {
        return Some("unbalanced parentheses".to_string());
    }

    if let Some(open_idx) = tool.find('(') {
        if !tool.ends_with(')') {
            return Some("invalid tool pattern: missing closing ')'".to_string());
        }
        let tool_name = &tool[..open_idx];
        if tool_name.trim().is_empty() {
            return Some("tool name cannot be empty".to_string());
        }
    }

    None
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
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyLicense)));
    }

    #[test]
    fn test_validate_license_wrong_type() {
        let mut metadata = base_metadata();
        metadata.insert("license".to_string(), Value::Number(123.into()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidType(f) if f == "license")));
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
            Value::String("Bash(git:*) Read Write".to_string()),
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
        assert!(errors.iter().any(
            |e| matches!(e, ValidationError::InvalidToolSpec { reason, .. } if reason.contains("unbalanced"))
        ));
    }

    #[test]
    fn test_validate_allowed_tools_lowercase_tool_name_is_allowed() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("mcp__playwright".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_allowed_tools_comma_delimited_rejected() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(git:*),Read".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(
            |e| matches!(e, ValidationError::InvalidToolSpec { reason, .. } if reason.contains("space-delimited"))
        ));
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

    #[test]
    fn test_validate_compatibility_empty() {
        let mut metadata = base_metadata();
        metadata.insert("compatibility".to_string(), Value::String(String::new()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyField(f) if f == "compatibility")));
    }

    #[test]
    fn test_validate_metadata_must_be_string_map() {
        let mut metadata = base_metadata();
        metadata.insert(
            "metadata".to_string(),
            Value::String("not-a-map".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MetadataNotMapping)));

        let mut map = serde_yaml::Mapping::new();
        map.insert(Value::Number(1.into()), Value::String("ok".to_string()));
        map.insert(
            Value::String("nested".to_string()),
            Value::Mapping(serde_yaml::Mapping::new()),
        );
        let mut metadata = base_metadata();
        metadata.insert("metadata".to_string(), Value::Mapping(map));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MetadataNonStringKey)));
        assert!(errors.iter().any(
            |e| matches!(e, ValidationError::MetadataNonStringValue { key } if key == "nested")
        ));
    }
}
