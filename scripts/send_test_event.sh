#!/usr/bin/env bash
# Envia um único evento sintético; não carrega o .env como código shell.
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# Mantém o diretório de execução, de onde o app também lê o .env.
exec cargo run --quiet --manifest-path "$project_root/Cargo.toml" --example send_test_event
