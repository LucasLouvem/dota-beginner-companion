#!/usr/bin/env bash
# Executa todas as verificações obrigatórias do projeto. Para no primeiro erro.
set -euo pipefail
cd "$(dirname "$0")/.."

run() {
    echo "==> $*"
    "$@"
}

run cargo fmt --check
run cargo clippy --all-targets -- -D warnings
run cargo test
run dprint check
run rumdl check .
run typos
echo "==> todas as verificações passaram"
