# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `--dry-run` / `-n` flag for `fix` command to preview changes without modifying files
- Validation for `license` field (must be non-empty string if provided)
- Validation for `allowed-tools` field (validates format and balanced parentheses)
- Proper error types with `thiserror` for better error handling
- Comprehensive module documentation
- More property-based tests for description validation

### Changed
- Split monolithic `lib.rs` into focused modules: `discovery`, `validation`, `formatting`, `fix`, `skill`, `error`
- Character length validation now correctly counts Unicode characters instead of bytes
- Fix mode no longer reports "Fixed" when metadata normalization makes no actual changes

### Fixed
- Character length limits now work correctly for non-ASCII text (Japanese, Cyrillic, etc.)
- `fix_skill` no longer reads the file twice (uses already-loaded content)
- Metadata normalization only sets `changed` flag when content actually changes

## [0.1.4] - 2025-01-15

### Added
- Comprehensive test suite with unit, integration, and property-based tests
- Fuzz targets for frontmatter parsing and metadata validation
- README execution tests (executable markdown)
- CI workflow with format, lint, test, and fuzz smoke tests

### Fixed
- README binary path in CI

## [0.1.3] - 2025-01-14

### Added
- Executable README with tested code blocks

## [0.1.2] - 2025-01-13

### Added
- Initial release with check and fix commands
- YAML frontmatter validation
- Skill naming rules enforcement
- NFKC Unicode normalization for international names
- Pre-commit hooks support
