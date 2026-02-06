//! A fast, spec-compliant linter and formatter for Agent Skills (`SKILL.md`).
//!
//! This crate provides tools for validating and fixing SKILL.md files, which are
//! used to define agent skills with YAML frontmatter metadata.
//!
//! # Features
//!
//! - Validates required YAML frontmatter and field constraints
//! - Enforces skill naming rules and directory/name matching (NFKC normalization)
//! - Fix mode normalizes formatting and repairs common issues
//! - Designed for pre-commit/prek hooks
//!
//! # Example
//!
//! ```rust,no_run
//! use agent_skills_lint::{collect_skill_files, check_skill};
//! use std::path::PathBuf;
//!
//! let skills = collect_skill_files(&[PathBuf::from("./skills")]);
//! for skill in skills {
//!     let errors = check_skill(&skill);
//!     if !errors.is_empty() {
//!         eprintln!("Errors in {}: {:?}", skill.file_path.display(), errors);
//!     }
//! }
//! ```

pub mod discovery;
pub mod error;
pub mod fix;
pub mod formatting;
pub mod skill;
pub mod validation;

// Re-export primary types and functions for convenience
pub use discovery::{collect_skill_files, display_path, find_skill_md, repo_root};
pub use error::{FixError, ParseError, ValidationError};
pub use fix::{check_skill, fix_skill, FixResult};
pub use formatting::{format_frontmatter, parse_frontmatter};
pub use skill::{
    SkillFile, ALLOWED_FIELDS, FIELD_ORDER, MAX_COMPATIBILITY_LENGTH, MAX_DESCRIPTION_LENGTH,
    MAX_SKILL_NAME_LENGTH,
};
pub use validation::validate_metadata;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use serde_yaml::{Mapping, Value};
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    fn temp_skill_dir(name: &str) -> TempDir {
        let dir = TempDir::new().expect("temp dir");
        let skill_dir = dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        dir
    }

    fn write_skill(dir: &std::path::Path, filename: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(filename);
        fs::write(&path, content).expect("write skill file");
        path
    }

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
    fn parse_frontmatter_valid() {
        let content = "---\nname: my-skill\ndescription: A test skill\n---\n# Title\n\nBody\n";
        let (metadata, body) = parse_frontmatter(content).expect("frontmatter parsed");
        assert_eq!(
            metadata.get("name"),
            Some(&Value::String("my-skill".to_string()))
        );
        assert_eq!(body, "# Title\n\nBody");
    }

    #[test]
    fn parse_frontmatter_bom() {
        let content = "\u{feff}---\nname: my-skill\ndescription: A test skill\n---\nBody";
        let (metadata, body) = parse_frontmatter(content).expect("frontmatter parsed");
        assert_eq!(
            metadata.get("name"),
            Some(&Value::String("my-skill".to_string()))
        );
        assert_eq!(body, "Body");
    }

    #[test]
    fn parse_frontmatter_allows_triple_dash_in_quoted_strings() {
        let content = "---\nname: my-skill\ndescription: \"contains --- in text\"\n---\nBody\n";
        let (metadata, body) = parse_frontmatter(content).expect("frontmatter parsed");
        assert_eq!(
            metadata.get("description"),
            Some(&Value::String("contains --- in text".to_string()))
        );
        assert_eq!(body, "Body");
    }

    #[test]
    fn parse_frontmatter_errors() {
        let missing = "No frontmatter here";
        assert!(matches!(
            parse_frontmatter(missing),
            Err(ParseError::MissingFrontmatter)
        ));

        let unclosed = "---\nname: my-skill\ndescription: A test skill\n";
        assert!(matches!(
            parse_frontmatter(unclosed),
            Err(ParseError::UnclosedFrontmatter)
        ));

        let invalid = "---\nname: [invalid\ndescription: broken\n---\nBody";
        assert!(matches!(
            parse_frontmatter(invalid),
            Err(ParseError::InvalidYaml(_))
        ));

        let non_mapping = "---\n- just\n- a\n- list\n---\nBody";
        assert!(matches!(
            parse_frontmatter(non_mapping),
            Err(ParseError::NotAMapping)
        ));
    }

    #[test]
    fn parse_frontmatter_non_string_key() {
        let content = "---\n1: a\nname: my-skill\ndescription: A test skill\n---\nBody";
        assert!(matches!(
            parse_frontmatter(content),
            Err(ParseError::NonStringKey)
        ));
    }

    #[test]
    fn find_skill_md_prefers_uppercase() {
        let dir = TempDir::new().expect("temp dir");
        let skill_dir = dir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("mkdir");

        // On case-insensitive filesystems (macOS/Windows), creating both files
        // just overwrites the same file. The new find_skill_md reads the actual
        // directory entry, so test that it returns whatever exists.
        write_skill(&skill_dir, "SKILL.md", "uppercase");

        let found = find_skill_md(&skill_dir).expect("find skill");
        assert_eq!(found.file_name().unwrap().to_string_lossy(), "SKILL.md");

        // Test that lowercase is also found when only lowercase exists
        fs::remove_file(skill_dir.join("SKILL.md")).expect("remove");
        write_skill(&skill_dir, "skill.md", "lowercase");
        let found = find_skill_md(&skill_dir).expect("find skill");
        // On case-insensitive FS, the name will be "skill.md"
        assert!(found
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq_ignore_ascii_case("skill.md"));
    }

    #[test]
    fn check_skill_missing_file() {
        let dir = temp_skill_dir("missing-skill");
        let skill_dir = dir.path().join("missing-skill");
        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_dir.join("SKILL.md"),
            content: String::new(),
        };
        let errors = check_skill(&skill);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::MissingFile(_))));
    }

    #[test]
    fn validate_metadata_missing_fields() {
        let mut metadata = base_metadata();
        metadata.remove("name");
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::MissingField(f) if f == "name")));

        let mut metadata = base_metadata();
        metadata.remove("description");
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::MissingField(f) if f == "description")));
    }

    #[test]
    fn validate_name_rules() {
        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("MySkill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameNotLowercase(_))));

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("-my-skill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameInvalidHyphen)));

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("my--skill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameConsecutiveHyphens)));

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("my_skill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameInvalidChars(_))));

        let long_name = "a".repeat(MAX_SKILL_NAME_LENGTH + 1);
        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String(long_name));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameTooLong { .. })));
    }

    #[test]
    fn validate_name_directory_match_and_i18n() {
        let dir = TempDir::new().expect("temp dir");
        let name = "caf\u{00e9}";
        let skill_dir = dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create dir");

        let mut metadata = base_metadata();
        metadata.insert(
            "name".to_string(),
            Value::String("cafe\u{0301}".to_string()),
        );
        let errors = validate_metadata(&metadata, Some(&skill_dir));
        assert!(errors.is_empty());

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("wrong-name".to_string()));
        let errors = validate_metadata(&metadata, Some(&skill_dir));
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameMismatch { .. })));
    }

    #[test]
    fn validate_i18n_case() {
        let dir = TempDir::new().expect("temp dir");
        let name = "\u{6280}\u{80fd}";
        let skill_dir = dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create dir");

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String(name.to_string()));
        let errors = validate_metadata(&metadata, Some(&skill_dir));
        assert!(errors.is_empty());

        let upper = "\u{041d}\u{0410}\u{0412}\u{042b}\u{041a}";
        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String(upper.to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NameNotLowercase(_))));
    }

    #[test]
    fn validate_description_and_compatibility_limits() {
        let mut metadata = base_metadata();
        metadata.insert(
            "description".to_string(),
            Value::String("x".repeat(MAX_DESCRIPTION_LENGTH + 1)),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::DescriptionTooLong { .. })));

        let mut metadata = base_metadata();
        metadata.insert(
            "compatibility".to_string(),
            Value::String("x".repeat(MAX_COMPATIBILITY_LENGTH + 1)),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::CompatibilityTooLong { .. })));

        let mut metadata = base_metadata();
        metadata.insert("compatibility".to_string(), Value::Number(1.into()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::InvalidType(f) if f == "compatibility")));

        let mut metadata = base_metadata();
        metadata.insert("compatibility".to_string(), Value::String(String::new()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::EmptyField(f) if f == "compatibility")));
    }

    #[test]
    fn validate_allowed_fields() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(jq:*)".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());

        let mut metadata = base_metadata();
        metadata.insert("owner".to_string(), Value::String("me".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::UnexpectedFields { .. })));
    }

    #[test]
    fn derive_description_skips_code_blocks() {
        use crate::formatting::derive_description;

        let body = "\n# Title\n\n```bash\necho hi\n```\n\nUse this skill to do X.\n";
        assert_eq!(derive_description(body), "Use this skill to do X.");

        let fallback = "# Title\n\n```bash\necho hi\n```\n";
        assert_eq!(derive_description(fallback), "Title");
    }

    #[test]
    fn format_frontmatter_orders_and_quotes() {
        use crate::formatting::format_frontmatter;

        let mut metadata = BTreeMap::new();
        metadata.insert(
            "description".to_string(),
            Value::String("Use: this # now".to_string()),
        );
        metadata.insert("name".to_string(), Value::String("my-skill".to_string()));
        metadata.insert("license".to_string(), Value::String("MIT".to_string()));
        metadata.insert(
            "compatibility".to_string(),
            Value::String("Rust 1.75+".to_string()),
        );
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(git:*)".to_string()),
        );
        let mut meta_map = Mapping::new();
        meta_map.insert(
            Value::String("z".to_string()),
            Value::String("2".to_string()),
        );
        meta_map.insert(
            Value::String("a".to_string()),
            Value::String("1".to_string()),
        );
        metadata.insert("metadata".to_string(), Value::Mapping(meta_map));
        metadata.insert("owner".to_string(), Value::String("me".to_string()));

        let formatted = format_frontmatter(&metadata).expect("format frontmatter");
        let expected = [
            "---",
            "name: \"my-skill\"",
            "description: \"Use: this # now\"",
            "license: \"MIT\"",
            "compatibility: \"Rust 1.75+\"",
            "allowed-tools: \"Bash(git:*)\"",
            "metadata:",
            "  a: \"1\"",
            "  z: \"2\"",
            "owner: \"me\"",
            "---",
        ]
        .join("\n");
        assert_eq!(formatted, expected);
    }

    #[test]
    fn check_and_fix_skill_files() {
        let dir = temp_skill_dir("my-skill");
        let skill_dir = dir.path().join("my-skill");
        let skill_path = write_skill(
            &skill_dir,
            "skill.md",
            "# Title\n\nUse this skill to do X.\n",
        );

        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_path,
            content: fs::read_to_string(skill_dir.join("skill.md")).unwrap(),
        };

        let errors = check_skill(&skill);
        assert!(errors
            .iter()
            .any(|err| matches!(err, ValidationError::NotUppercase)));

        let result = fix_skill(&skill, false);
        assert!(result.changed);
        assert!(result.errors.is_empty());

        let fixed = fs::read_to_string(skill_dir.join("SKILL.md")).expect("read fixed");
        assert!(fixed.contains("name: \"my-skill\""));
        assert!(fixed.contains("description: \"Use this skill to do X.\""));
        assert!(fixed.ends_with('\n'));

        let fixed_skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_dir.join("SKILL.md"),
            content: fixed,
        };
        let errors = check_skill(&fixed_skill);
        assert!(errors.is_empty());
    }

    #[test]
    fn fix_skill_preserves_string_scalars_that_look_like_bools() {
        let dir = temp_skill_dir("true-skill");
        let skill_dir = dir.path().join("true-skill");
        let content = r#"---
name: true-skill
description: "true"
---
Body
"#;
        let skill_path = write_skill(&skill_dir, "SKILL.md", content);
        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_path,
            content: fs::read_to_string(skill_dir.join("SKILL.md")).unwrap(),
        };

        let result = fix_skill(&skill, false);
        assert!(result.changed);
        assert!(result.errors.is_empty());

        let fixed = fs::read_to_string(skill_dir.join("SKILL.md")).expect("read fixed");
        assert!(fixed.contains("description: \"true\""));

        let fixed_skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_dir.join("SKILL.md"),
            content: fixed,
        };
        let errors = check_skill(&fixed_skill);
        assert!(errors.is_empty());
    }

    #[test]
    fn fix_skill_preserves_unknown_fields_and_metadata() {
        let dir = temp_skill_dir("my-skill");
        let skill_dir = dir.path().join("my-skill");
        let content = r"---
description: A test skill
metadata:
  version: 1.2
  count: 5
owner: team
---
# Title
";
        let skill_path = write_skill(&skill_dir, "SKILL.md", content);
        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_path,
            content: fs::read_to_string(skill_dir.join("SKILL.md")).unwrap(),
        };

        let result = fix_skill(&skill, false);
        assert!(result.changed);
        assert!(result.errors.is_empty());

        let fixed = fs::read_to_string(skill_dir.join("SKILL.md")).expect("read fixed");
        assert!(fixed.contains("name: \"my-skill\""));
        assert!(fixed.contains("description: \"A test skill\""));
        assert!(fixed.contains("metadata:"), "fixed content:\n{fixed}");
        assert!(fixed.contains("  count: \"5\""));
        assert!(fixed.contains("  version: \"1.2\""));
        assert!(fixed.contains("owner: \"team\""));
    }

    #[test]
    fn collect_skill_files_discovers_nested() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        let skill_a = root.join("a").join("SKILL.md");
        let skill_b = root.join("b").join("skill.md");
        fs::create_dir_all(skill_a.parent().unwrap()).expect("mkdir");
        fs::create_dir_all(skill_b.parent().unwrap()).expect("mkdir");
        fs::write(&skill_a, "---\nname: a\ndescription: A\n---\nBody\n").expect("write");
        fs::write(&skill_b, "---\nname: b\ndescription: B\n---\nBody\n").expect("write");

        let paths = vec![root.to_path_buf()];
        let skills = collect_skill_files(&paths);
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn fix_skill_dry_run() {
        let dir = temp_skill_dir("my-skill");
        let skill_dir = dir.path().join("my-skill");
        let skill_path = write_skill(
            &skill_dir,
            "skill.md",
            "# Title\n\nUse this skill to do X.\n",
        );

        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_path,
            content: fs::read_to_string(skill_dir.join("skill.md")).unwrap(),
        };

        let result = fix_skill(&skill, true);
        assert!(result.changed);
        assert!(result.new_content.is_some());

        // File should still be skill.md (not renamed in dry-run)
        // Note: On case-insensitive filesystems (macOS), we check the actual filename
        let entries: Vec<_> = fs::read_dir(&skill_dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries.iter().any(|name| name == "skill.md"),
            "Expected skill.md to still exist (not renamed to SKILL.md), found: {entries:?}"
        );
    }

    proptest! {
        #[test]
        fn prop_valid_names_are_accepted(name in "[a-z0-9]{1,8}(-[a-z0-9]{1,8}){0,6}") {
            let mut metadata = base_metadata();
            metadata.insert("name".to_string(), Value::String(name));
            let errors = validate_metadata(&metadata, None);
            // Valid names should not produce name-related errors
            let has_name_error = errors.iter().any(|err| matches!(err,
                ValidationError::NameTooLong { .. } |
                ValidationError::NameNotLowercase(_) |
                ValidationError::NameInvalidHyphen |
                ValidationError::NameConsecutiveHyphens |
                ValidationError::NameInvalidChars(_)
            ));
            prop_assert!(!has_name_error);
        }

        #[test]
        fn prop_invalid_names_with_uppercase_fail(name in "[A-Z]{1,10}") {
            let mut metadata = base_metadata();
            metadata.insert("name".to_string(), Value::String(name));
            let errors = validate_metadata(&metadata, None);
            let has_lowercase_error = errors.iter().any(|err| matches!(err, ValidationError::NameNotLowercase(_)));
            prop_assert!(has_lowercase_error);
        }

        #[test]
        fn prop_valid_descriptions_pass(desc in ".{1,100}") {
            if desc.trim().is_empty() {
                return Ok(());
            }
            let mut metadata = base_metadata();
            metadata.insert("description".to_string(), Value::String(desc));
            let errors = validate_metadata(&metadata, None);
            let has_desc_error = errors.iter().any(|err| matches!(err, ValidationError::DescriptionTooLong { .. }));
            prop_assert!(!has_desc_error);
        }

        #[test]
        fn prop_long_descriptions_fail(desc in ".{1025,1100}") {
            let mut metadata = base_metadata();
            metadata.insert("description".to_string(), Value::String(desc));
            let errors = validate_metadata(&metadata, None);
            let has_desc_error = errors.iter().any(|err| matches!(err, ValidationError::DescriptionTooLong { .. }));
            prop_assert!(has_desc_error);
        }
    }
}
