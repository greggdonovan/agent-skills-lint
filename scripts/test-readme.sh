#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
bin="${AGENT_SKILLS_LINT_BIN:-$repo_root/target/debug/agent-skills-lint}"

if [ ! -x "$bin" ]; then
  (cd "$repo_root" && cargo build -q)
fi

AGENT_SKILLS_LINT_BIN="$bin" bash "$repo_root/README.md"
