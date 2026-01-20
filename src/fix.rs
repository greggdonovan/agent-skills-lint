//! Check and fix operations for skill files.
//!
//! This module provides the core validation and auto-fix logic for SKILL.md files.
//! The `check_skill` function validates a skill against the spec, while `fix_skill`
//! can automatically repair common issues like missing frontmatter or mismatched names.

use std::collections::BTreeMap;
use std::fs;

use serde_yaml::{Mapping, Value};
use unicode_normalization::UnicodeNormalization;

use crate::discovery::{find_skill_md, get_dir_name};
use crate::formatting::{
    derive_description, format_frontmatter, mapping_to_string_map, parse_frontmatter,
};
use crate::skill::SkillFile;
use crate::validation::validate_metadata;

/// Result of a fix operation.
#[derive(Debug)]
pub struct FixResult {
    /// Whether any changes were made.
    pub changed: bool,
    /// Any errors that occurred.
    pub errors: Vec<String>,
    /// The new content (for dry-run mode).
    pub new_content: Option<String>,
    /// The target path (may differ from original if renamed).
    pub target_path: Option<std::path::PathBuf>,
}

/// Check a skill file for validation errors.
///
/// Returns a list of validation errors. An empty list indicates the skill is valid.
pub fn check_skill(skill: &SkillFile) -> Vec<String> {
    let mut errors = Vec::new();

    if !skill.dir_path.exists() {
        errors.push(format!("Path does not exist: {}", skill.dir_path.display()));
        return errors;
    }

    if !skill.dir_path.is_dir() {
        errors.push(format!("Not a directory: {}", skill.dir_path.display()));
        return errors;
    }

    if !skill.file_path.exists() {
        errors.push("Missing required file: SKILL.md".to_string());
        return errors;
    }

    if skill.file_path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
        errors.push("SKILL.md should be uppercase".to_string());
    }

    match parse_frontmatter(&skill.content) {
        Ok((metadata, _body)) => {
            errors.extend(validate_metadata(&metadata, Some(&skill.dir_path)));
        }
        Err(err) => errors.push(err.to_string()),
    }

    errors
}

/// Fix a skill file by normalizing its format and content.
///
/// This will:
/// - Rename `skill.md` to `SKILL.md` if needed
/// - Strip UTF-8 BOM if present
/// - Generate frontmatter if missing
/// - Fix the name field to match the directory
/// - Generate description if missing
/// - Normalize the metadata field
///
/// If `dry_run` is true, returns the new content without writing to disk.
pub fn fix_skill(skill: &SkillFile, dry_run: bool) -> FixResult {
    let mut errors = Vec::new();
    let mut changed = false;

    if !skill.dir_path.exists() || !skill.dir_path.is_dir() {
        return FixResult {
            changed: false,
            errors: vec![format!("Not a directory: {}", skill.dir_path.display())],
            new_content: None,
            target_path: None,
        };
    }

    let mut skill_path = find_skill_md(&skill.dir_path).unwrap_or_else(|| skill.file_path.clone());

    // Handle file rename (skill.md -> SKILL.md)
    let needs_rename = skill_path.exists()
        && skill_path
            .file_name()
            .map(|n| n.eq_ignore_ascii_case("skill.md"))
            .unwrap_or(false)
        && skill_path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md");

    if needs_rename {
        let new_path = skill_path.with_file_name("SKILL.md");
        if !dry_run {
            if let Err(err) = fs::rename(&skill_path, &new_path) {
                return FixResult {
                    changed: false,
                    errors: vec![format!("Failed to rename {}: {err}", skill_path.display())],
                    new_content: None,
                    target_path: None,
                };
            }
        }
        skill_path = new_path;
        changed = true;
    }

    if !dry_run && !skill_path.exists() {
        return FixResult {
            changed,
            errors: vec!["Missing required file: SKILL.md".to_string()],
            new_content: None,
            target_path: Some(skill_path),
        };
    }

    // Use the already-loaded content instead of reading again
    let mut content = skill.content.clone();

    if content.starts_with('\u{feff}') {
        content = content.trim_start_matches('\u{feff}').to_string();
        changed = true;
    }

    let mut metadata: BTreeMap<String, Value>;
    let body: String;
    let dir_name = get_dir_name(&skill.dir_path);

    if content.starts_with("---") {
        match parse_frontmatter(&content) {
            Ok((parsed, parsed_body)) => {
                metadata = parsed;
                body = parsed_body.trim_matches('\n').to_string();
            }
            Err(err) => {
                errors.push(err.to_string());
                return FixResult {
                    changed,
                    errors,
                    new_content: None,
                    target_path: Some(skill_path),
                };
            }
        }

        let dir_name_norm: String = dir_name.nfkc().collect();

        match metadata.get("name") {
            Some(Value::String(name)) if !name.trim().is_empty() => {
                let name_norm: String = name.trim().nfkc().collect();
                if name_norm != dir_name_norm {
                    metadata.insert("name".to_string(), Value::String(dir_name));
                    changed = true;
                }
            }
            _ => {
                metadata.insert("name".to_string(), Value::String(dir_name));
                changed = true;
            }
        }

        match metadata.get("description") {
            Some(Value::String(desc)) if !desc.trim().is_empty() => {}
            _ => {
                metadata.insert(
                    "description".to_string(),
                    Value::String(derive_description(&body)),
                );
                changed = true;
            }
        }

        if let Some(Value::Mapping(map)) = metadata.get_mut("metadata") {
            if let Ok(normalized) = mapping_to_string_map(map) {
                let mut new_map = Mapping::new();
                for (key, value) in normalized {
                    new_map.insert(Value::String(key), Value::String(value));
                }
                if *map != new_map {
                    *map = new_map;
                    changed = true;
                }
            }
        }
    } else {
        metadata = BTreeMap::new();
        metadata.insert("name".to_string(), Value::String(dir_name));
        metadata.insert(
            "description".to_string(),
            Value::String(derive_description(&content)),
        );
        body = content.trim_matches('\n').to_string();
        changed = true;
    }

    metadata.retain(|_, value| !matches!(value, Value::Null));

    let formatted = match format_frontmatter(&metadata) {
        Ok(result) => result,
        Err(err) => {
            errors.push(err);
            return FixResult {
                changed,
                errors,
                new_content: None,
                target_path: Some(skill_path),
            };
        }
    };

    let mut new_content = format!("{formatted}\n\n{body}");
    new_content = new_content.trim_end().to_string();
    new_content.push('\n');

    if new_content != content {
        changed = true;
    }

    if changed && !dry_run {
        if let Err(err) = fs::write(&skill_path, &new_content) {
            errors.push(format!("Failed to write {}: {err}", skill_path.display()));
            return FixResult {
                changed,
                errors,
                new_content: Some(new_content),
                target_path: Some(skill_path),
            };
        }
    }

    FixResult {
        changed,
        errors,
        new_content: if dry_run { Some(new_content) } else { None },
        target_path: Some(skill_path),
    }
}

/// Legacy wrapper for `fix_skill` that matches the old API.
///
/// Returns (changed, errors) tuple for backward compatibility.
pub fn fix_skill_compat(skill: &SkillFile) -> (bool, Vec<String>) {
    let result = fix_skill(skill, false);
    (result.changed, result.errors)
}
