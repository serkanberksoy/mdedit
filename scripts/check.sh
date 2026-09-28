#!/usr/bin/env bash
# Quality gate: must pass before a feature is considered done.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

echo "==> cargo fmt --check"
cargo fmt --check
echo "==> cargo clippy --all-targets -- -D warnings"
cargo clippy --quiet --all-targets -- -D warnings
echo "==> cargo test"
cargo test --quiet
echo "OK: format, lints and tests all pass"
