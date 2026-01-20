//! Core skill types and constants.

use std::path::PathBuf;

/// Maximum length for skill names (in characters).
pub const MAX_SKILL_NAME_LENGTH: usize = 64;

/// Maximum length for descriptions (in characters).
pub const MAX_DESCRIPTION_LENGTH: usize = 1024;

/// Maximum length for compatibility notes (in characters).
pub const MAX_COMPATIBILITY_LENGTH: usize = 500;

/// Fields allowed in SKILL.md frontmatter.
pub const ALLOWED_FIELDS: [&str; 6] = [
    "name",
    "description",
    "license",
    "allowed-tools",
    "metadata",
    "compatibility",
];

/// Order in which fields are written in formatted output.
pub const FIELD_ORDER: [&str; 6] = [
    "name",
    "description",
    "license",
    "compatibility",
    "allowed-tools",
    "metadata",
];

/// Represents a skill file with its location and content.
#[derive(Debug, Clone)]
pub struct SkillFile {
    /// Path to the directory containing the skill.
    pub dir_path: PathBuf,
    /// Path to the SKILL.md file itself.
    pub file_path: PathBuf,
    /// The file content.
    pub content: String,
}
