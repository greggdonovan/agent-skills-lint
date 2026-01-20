# agent-skills-lint

Fast linter and formatter for Agent Skills (`SKILL.md`) files. The rules mirror the Agent Skills reference spec (YAML frontmatter, required fields, naming constraints), with an optional fix mode to normalize formatting.

## Install

```bash
cargo install --git https://github.com/greggdonovan/agent-skills-lint
```

## Usage

Check one or more skills:

```bash
agent-skills-lint check path/to/skill
```

Fix formatting issues:

```bash
agent-skills-lint fix path/to/skill
```

If no paths are provided, the tool scans the repo for `SKILL.md` files.

## Prek / pre-commit

This repo ships a `.pre-commit-hooks.yaml` with two hooks:

- `agent-skills-lint` (check)
- `agent-skills-lint-fix` (manual fix)

Example config:

```yaml
repos:
  - repo: https://github.com/greggdonovan/agent-skills-lint
    rev: v0.1.0
    hooks:
      - id: agent-skills-lint
      - id: agent-skills-lint-fix
        stages: [manual]
```

Notes:

- These hooks use `language: system`, so the `agent-skills-lint` binary must be on `PATH`.
- For local development, you can also use `entry: cargo run --quiet -- check`.

## Exit codes

- `0` when all skills are valid
- `1` when any errors are found
