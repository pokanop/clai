#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

git config core.hooksPath .githooks
printf '%s\n' "Git hooks path set to .githooks (repo-local). pre-commit runs cargo fmt and re-stages changed .rs files."
