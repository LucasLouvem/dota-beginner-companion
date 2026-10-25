//! Leitura das fixtures de eventos do GSI em `tests/fixtures/gsi/`.

use serde_json::Value;

/// Partida real contra bots, do menu ao início da partida (9 eventos).
pub const ZEUS_MATCH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gsi/zeus_bot_match.jsonl"
));

/// Partida real contra bots, já em andamento (6 eventos).
pub const DRAGON_KNIGHT_MATCH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gsi/dragon_knight_bot_match.jsonl"
));

/// Fim real de uma partida contra bots (5 eventos): montagem do Satanic,
/// pós-jogo com o ID ainda `"0"`, pós-jogo com o ID e volta ao menu.
pub const DRAGON_KNIGHT_MATCH_END: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gsi/dragon_knight_match_end.jsonl"
));

/// Payloads de uma fixture, na ordem de chegada.
pub fn fixture_payloads(fixture: &str) -> Vec<Value> {
    fixture
        .lines()
        .map(|line| {
            let mut entry: Value = serde_json::from_str(line).expect("a fixture tem JSON válido");
            entry["payload"].take()
        })
        .collect()
}
