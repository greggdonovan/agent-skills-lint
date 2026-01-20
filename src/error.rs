//! Error types for agent-skills-lint.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when parsing SKILL.md frontmatter.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The file does not start with YAML frontmatter delimiter.
    #[error("SKILL.md must start with YAML frontmatter (---)")]
    MissingFrontmatter,

    /// The frontmatter is not properly closed with a second delimiter.
    #[error("SKILL.md frontmatter not properly closed with ---")]
    UnclosedFrontmatter,

    /// The YAML in the frontmatter is invalid.
    #[error("Invalid YAML in frontmatter: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),

    /// The frontmatter is not a YAML mapping.
    #[error("SKILL.md frontmatter must be a YAML mapping")]
    NotAMapping,

    /// A frontmatter key is not a string.
    #[error("Frontmatter keys must be strings")]
    NonStringKey,
}

/// Errors that can occur when fixing a skill file.
#[derive(Debug, Error)]
pub enum FixError {
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
    ParseError(#[from] ParseError),

    /// Unsupported YAML value type.
    #[error("Unsupported YAML value type for formatting")]
    UnsupportedValueType,
}

/// A validation error with details about what failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The error message.
    pub message: String,
}

impl ValidationError {
    /// Create a new validation error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ValidationError {}
