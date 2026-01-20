# Contributing to agent-skills-lint

Thank you for your interest in contributing! This document provides guidelines for contributing to the project.

## Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/greggdonovan/agent-skills-lint
   cd agent-skills-lint
   ```

2. **Build the project:**
   ```bash
   cargo build
   ```

3. **Run the tests:**
   ```bash
   cargo test
   ```

## Code Quality

Before submitting a pull request, ensure your code passes all checks:

```bash
# Format check
cargo fmt -- --check

# Lint with strict warnings
cargo clippy -- -D warnings

# Run all tests
cargo test

# Run README as executable tests
bash README.md
```

## Project Structure

```
src/
├── lib.rs          # Main library with re-exports
├── main.rs         # CLI entry point
├── discovery.rs    # Skill file discovery (git ls-files, walkdir)
├── validation.rs   # Metadata validation rules
├── formatting.rs   # Frontmatter parsing and formatting
├── fix.rs          # Check and fix logic
├── skill.rs        # Core types and constants
└── error.rs        # Error types
tests/
├── cli.rs          # CLI integration tests
└── readme.rs       # README execution tests
fuzz/
└── fuzz_targets/   # Fuzzing targets (requires nightly)
```

## Running Fuzz Tests

Fuzzing requires nightly Rust:

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run parse_frontmatter
cargo +nightly fuzz run validate_metadata
```

## Pull Request Process

1. Fork the repository and create a feature branch
2. Make your changes with clear commit messages
3. Add tests for new functionality
4. Ensure all checks pass (`cargo fmt`, `cargo clippy`, `cargo test`)
5. Update CHANGELOG.md with your changes under `[Unreleased]`
6. Submit a pull request with a clear description

## Commit Messages

- Use the imperative mood ("Add feature" not "Added feature")
- Keep the first line under 72 characters
- Reference issues where applicable

## Code Style

- Follow Rust idioms and conventions
- Use `rustfmt` for formatting
- Prefer explicit error handling over panics
- Add doc comments for public API functions
- Keep functions focused and testable

## Reporting Issues

When reporting issues, please include:
- Rust version (`rustc --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Sample SKILL.md file (if applicable)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
