//! Error types for agent-skills-lint.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when parsing SKILL.md frontmatter.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The file does not start with YAML frontmatter delimiter.
    #[error("SKILL.md must start with YAML frontmatter (---)")]
    MissingFrontmatter,

    /// The frontmatter is not properly closed with a second delimiter.
    #[error("SKILL.md frontmatter not properly closed with ---")]
    UnclosedFrontmatter,

    /// The YAML in the frontmatter is invalid.
    #[error("Invalid YAML in frontmatter: {0}")]
    InvalidYaml(String),

    /// The frontmatter is not a YAML mapping.
    #[error("SKILL.md frontmatter must be a YAML mapping")]
    NotAMapping,

    /// A frontmatter key is not a string.
    #[error("Frontmatter keys must be strings")]
    NonStringKey,
}

impl From<serde_yaml::Error> for ParseError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::InvalidYaml(err.to_string())
    }
}

/// Errors that can occur when fixing a skill file.
#[derive(Debug, Error)]
pub enum FixError {
    /// Path does not exist.
    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),

    /// The path is not a directory.
    #[error("Not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Failed to rename the file.
    #[error("Failed to rename {path}: {source}")]
    RenameFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The required SKILL.md file is missing.
    #[error("Missing required file: SKILL.md")]
    MissingFile,

    /// Failed to write the file.
    #[error("Failed to write {path}: {source}")]
    WriteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse the frontmatter.
    #[error("{0}")]
    Parse(#[from] ParseError),

    /// Unsupported YAML value type.
    #[error("Unsupported YAML value type for formatting")]
    UnsupportedValueType,
}

/// Validation errors for skill metadata.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Missing required file.
    #[error("Missing required file: {0}")]
    MissingFile(String),

    /// Path does not exist.
    #[error("Path does not exist: {0}")]
    PathNotFound(String),

    /// Path is not a directory.
    #[error("Not a directory: {0}")]
    NotADirectory(String),

    /// File should be uppercase SKILL.md.
    #[error("SKILL.md should be uppercase")]
    NotUppercase,

    /// Failed to parse frontmatter.
    #[error("{0}")]
    Parse(#[from] ParseError),

    /// Missing required field.
    #[error("Missing required field in frontmatter: {0}")]
    MissingField(String),

    /// Field must be a non-empty string.
    #[error("Field '{0}' must be a non-empty string")]
    EmptyField(String),

    /// Field must be a string type.
    #[error("Field '{0}' must be a string")]
    InvalidType(String),

    /// Metadata must be a mapping of string keys to string values.
    #[error("Field 'metadata' must be a mapping of string keys to string values")]
    MetadataNotMapping,

    /// Metadata key is not a string.
    #[error("Field 'metadata' contains a non-string key")]
    MetadataNonStringKey,

    /// Metadata value is not a string.
    #[error("Field 'metadata.{key}' must be a string")]
    MetadataNonStringValue { key: String },

    /// Skill name exceeds length limit.
    #[error("Skill name '{name}' exceeds {limit} character limit ({actual} chars)")]
    NameTooLong {
        name: String,
        limit: usize,
        actual: usize,
    },

    /// Skill name must be lowercase.
    #[error("Skill name '{0}' must be lowercase")]
    NameNotLowercase(String),

    /// Skill name has invalid hyphen placement.
    #[error("Skill name cannot start or end with a hyphen")]
    NameInvalidHyphen,

    /// Skill name has consecutive hyphens.
    #[error("Skill name cannot contain consecutive hyphens")]
    NameConsecutiveHyphens,

    /// Skill name contains invalid characters.
    #[error("Skill name '{0}' contains invalid characters. Only letters, digits, and hyphens are allowed.")]
    NameInvalidChars(String),

    /// Directory name doesn't match skill name.
    #[error("Directory name '{dir}' must match skill name '{name}'")]
    NameMismatch { dir: String, name: String },

    /// Description exceeds length limit.
    #[error("Description exceeds {limit} character limit ({actual} chars)")]
    DescriptionTooLong { limit: usize, actual: usize },

    /// Compatibility exceeds length limit.
    #[error("Compatibility exceeds {limit} character limit ({actual} chars)")]
    CompatibilityTooLong { limit: usize, actual: usize },

    /// License must be non-empty if provided.
    #[error("Field 'license' must be a non-empty string if provided")]
    EmptyLicense,

    /// Invalid tool specification format.
    #[error("Invalid tool specification '{spec}': {reason}")]
    InvalidToolSpec { spec: String, reason: String },

    /// Allowed-tools array item must be a string.
    #[error("Field 'allowed-tools' array item {index} must be a string")]
    InvalidToolArrayItem { index: usize },

    /// Allowed-tools must be string or array.
    #[error("Field 'allowed-tools' must be a string or array of strings")]
    InvalidToolsType,

    /// Unexpected fields in frontmatter.
    #[error("Unexpected fields in frontmatter: {fields}. Only {allowed:?} are allowed.")]
    UnexpectedFields {
        fields: String,
        allowed: &'static [&'static str],
    },
}
