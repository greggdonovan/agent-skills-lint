# agent-skills-lint
    # This is executable Markdown that's tested on CI.
    # How is that possible? See https://gist.github.com/bwoods/1c25cb7723a06a076c2152a2781d4d49
    set -o errexit -o nounset -o pipefail
    shopt -s expand_aliases
    alias ~~~=":<<'~~~sh'";:<<'~~~sh'

[![CI](https://github.com/greggdonovan/agent-skills-lint/actions/workflows/ci.yml/badge.svg)](https://github.com/greggdonovan/agent-skills-lint/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/agent-skills-lint.svg)](https://crates.io/crates/agent-skills-lint)
[![Documentation](https://docs.rs/agent-skills-lint/badge.svg)](https://docs.rs/agent-skills-lint)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.92-blue.svg)](https://blog.rust-lang.org/)

Fast, spec-compliant linter and formatter for Agent Skills (`SKILL.md`).

- Validates required YAML frontmatter and field constraints
- Enforces skill naming rules and directory/name matching (NFKC)
- Fix mode normalizes formatting and repairs common issues
- Designed for pre-commit/prek hooks

## Quickstart (tested)
~~~sh
set -o errexit -o nounset -o pipefail

repo_root=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
bin="${AGENT_SKILLS_LINT_BIN:-$repo_root/target/debug/agent-skills-lint}"
case "$bin" in
  /*) ;;
  *) bin="$repo_root/$bin" ;;
esac

if [ ! -x "$bin" ]; then
  (cd "$repo_root" && cargo build -q)
fi

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"

mkdir -p skills/good-skill
cat > skills/good-skill/SKILL.md <<'DOC'
---
name: good-skill
description: Demonstrates a properly formatted skill.
---
# Good Skill
DOC

"$bin" check skills/good-skill

mkdir -p skills/bad-skill
cat > skills/bad-skill/skill.md <<'DOC'
# Bad Skill

This skill is missing frontmatter.
DOC

"$bin" fix skills/bad-skill
"$bin" check skills/bad-skill
~~~

## Install

### From git (today)

```bash
cargo install --git https://github.com/greggdonovan/agent-skills-lint
```

### From crates.io (soon)

```bash
# cargo install agent-skills-lint
```

## Usage

```bash
agent-skills-lint check path/to/skill
agent-skills-lint fix path/to/skill
agent-skills-lint fix --dry-run path/to/skill  # Preview changes without modifying files
```

If no paths are provided, the tool scans the repo for `SKILL.md` files.

## Prek / pre-commit

This repo ships `.pre-commit-hooks.yaml` with two hooks:

- `agent-skills-lint` (check)
- `agent-skills-lint-fix` (manual fix)

Example config:

```yaml
repos:
  - repo: https://github.com/greggdonovan/agent-skills-lint
    rev: v0.1.4
    hooks:
      - id: agent-skills-lint
      - id: agent-skills-lint-fix
        stages: [manual]
```

Notes:

- These hooks use `language: system`, so the `agent-skills-lint` binary must be on `PATH`.
- For local development, you can also use `entry: cargo run --quiet -- check`.

## Rules enforced

- YAML frontmatter is required and must be a mapping.
- Required fields: `name`, `description`.
- Optional fields: `license`, `compatibility`, `allowed-tools`, `metadata`.
- `name` must be lowercase, ≤64 chars, alnum + hyphen, no leading/trailing hyphen, no consecutive hyphens.
- `name` must match the directory name (after NFKC normalization).
- `description` ≤1024 chars.
- `compatibility` ≤500 chars.
- `metadata` keys/values are stringified; unknown fields are preserved but reported.

## Exit codes

- `0` when all skills are valid
- `1` when any errors are found

## Testing

```bash
cargo test
cargo fmt -- --check
cargo clippy -- -D warnings
```

### README tests

`README.md` is executable. Run:

```bash
bash README.md
```

Only code blocks fenced with `~~~sh` are executed; everything else is ignored.
This is run in CI to keep the README accurate.

### Fuzzing

This repo includes `cargo-fuzz` targets for frontmatter parsing and metadata validation.

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run parse_frontmatter
cargo +nightly fuzz run validate_metadata
```

Notes:

- `cargo-fuzz` uses nightly Rust.
- Fuzz targets live under `fuzz/fuzz_targets`.

## License

MIT
