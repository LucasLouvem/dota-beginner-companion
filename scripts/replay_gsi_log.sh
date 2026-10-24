#!/usr/bin/env bash
# Reenvia ao app aberto os eventos gravados em logs/gsi.jsonl; aceita os mesmos
# argumentos do exemplo (arquivo e --interval-ms).
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# Mantém o diretório de execução, de onde o app também lê o .env.
exec cargo run --quiet --manifest-path "$project_root/Cargo.toml" --example replay_gsi_log -- "$@"
