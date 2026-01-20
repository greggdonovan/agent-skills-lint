# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Agent-skills-lint is a Rust CLI tool that lints and formats Agent Skills (`SKILL.md` files). It validates YAML frontmatter, enforces skill naming conventions, and provides auto-fix capabilities.

## Build & Test Commands

```bash
# Build
cargo build -q                    # Debug build
cargo build --release             # Release build

# Run all checks (CI pipeline)
cargo fmt -- --check              # Format check
cargo clippy -- -D warnings       # Lint with strict warnings
cargo test                        # Unit + integration tests
bash README.md                    # Execute README as tests

# Run single test
cargo test <test_name>            # e.g., cargo test test_parse_frontmatter_valid

# Fuzzing (requires nightly)
cargo +nightly fuzz run parse_frontmatter
cargo +nightly fuzz run validate_metadata
```

## Architecture

The codebase is organized into focused modules:

```
src/
├── lib.rs          # Main library with re-exports and tests
├── main.rs         # CLI entry point (clap)
├── discovery.rs    # Skill file discovery (git ls-files, walkdir)
├── validation.rs   # Metadata validation rules
├── formatting.rs   # Frontmatter parsing and formatting
├── fix.rs          # Check and fix logic
├── skill.rs        # Core types (SkillFile) and constants
└── error.rs        # Error types (ParseError, FixError, ValidationError)
```

**Key flows:**
- **Discovery**: `collect_skill_files()` → `discover_skills()` (uses git ls-files with WalkDir fallback)
- **Validation**: `parse_frontmatter()` → `validate_metadata()` (accumulates all errors, not fail-first)
- **Fix mode**: `fix_skill(skill, dry_run)` normalizes formatting, auto-generates missing fields, renames skill.md → SKILL.md

**Validation rules:**
- Required fields: `name`, `description`
- Name: lowercase, 1-64 chars, alphanumeric+hyphen only, must match directory name after NFKC normalization
- Description: 1-1024 chars
- License: optional, must be non-empty string if provided
- Compatibility: optional, max 500 chars
- Allowed-tools: optional, validates tool specification format

**Exit codes:** 0 = success, 1 = errors or no SKILL.md files found

## CLI Usage

```bash
agent-skills-lint check path/to/skill     # Validate
agent-skills-lint fix path/to/skill       # Auto-fix
agent-skills-lint fix -n path/to/skill    # Dry-run (preview changes)
```

## Testing Notes

- README.md is executable: code blocks in `~~~sh` fences are tested in CI
- Integration tests in `tests/cli.rs` test full CLI behavior with tempfile
- Property-based tests use proptest for name and description validation
- Fuzz targets in `fuzz/fuzz_targets/` test parser robustness
- Tests handle macOS case-insensitive filesystem quirks
