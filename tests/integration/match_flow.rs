//! Sequências reais de eventos do GSI (fixtures anonimizadas) passando pelo
//! normalizador e pelo acompanhamento da partida. A lista esperada foi escrita
//! à mão a partir da leitura das fixtures.

use dota_companion::domain::match_events::{
    ItemPlacement, MatchEvent, MatchEventKind, MatchResult, MatchTracker, Provenance,
};
use dota_companion::gsi::normalize::game_snapshot;
use serde_json::Value;

fn fixture(name: &str) -> Vec<Value> {
    let path = format!("{}/tests/fixtures/gsi/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .expect("a fixture existe")
        .lines()
        .map(|line| {
            let mut entry: Value = serde_json::from_str(line).expect("a fixture tem JSON válido");
            entry["payload"].take()
        })
        .collect()
}

/// Eventos de cada payload, na ordem de chegada.
fn events_per_payload(name: &str) -> Vec<Vec<MatchEvent>> {
    let mut tracker = MatchTracker::default();
    fixture(name)
        .iter()
        .map(|payload| tracker.observe(game_snapshot(payload)))
        .collect()
}

fn confirmed(kind: MatchEventKind) -> MatchEvent {
    MatchEvent {
        kind,
        provenance: Provenance::Confirmed,
    }
}

fn inferred(kind: MatchEventKind) -> MatchEvent {
    MatchEvent {
        kind,
        provenance: Provenance::Inferred,
    }
}

fn leveled(ability: &str, level: u32) -> MatchEvent {
    confirmed(MatchEventKind::AbilityLeveled {
        ability: ability.into(),
        level,
    })
}

#[test]
fn zeus_match_from_menu_to_start_produces_expected_events_in_order() {
    let events = events_per_payload("zeus_bot_match.jsonl");

    assert_eq!(
        events,
        vec![
            // Menu, sem `map`.
            vec![],
            // Seleção de heróis vinda do menu, ainda com `hero.id: 0`.
            vec![inferred(MatchEventKind::MatchFound)],
            // Herói escolhido no tempo de estratégia.
            vec![confirmed(MatchEventKind::HeroPicked {
                hero: "npc_dota_hero_zuus".into(),
            })],
            // Compras na estratégia: só o ouro cai; itens ainda não aparecem.
            vec![],
            vec![],
            // Apresentação e carregamento, com o relógio voltando.
            vec![],
            vec![],
            // Pré-jogo: o inventário aparece pela primeira vez, sem eventos de item.
            vec![],
            // Início da partida e o primeiro ponto de habilidade.
            vec![
                confirmed(MatchEventKind::MatchStarted),
                leveled("zuus_arc_lightning", 1),
            ],
        ]
    );
}

#[test]
fn dragon_knight_match_in_progress_produces_level_item_and_death_events() {
    let events = events_per_payload("dragon_knight_bot_match.jsonl");

    assert_eq!(
        events,
        vec![
            // Primeiro evento já em andamento: não é um início de partida.
            vec![],
            // Nível do herói subiu; nível do herói não é evento.
            vec![],
            vec![leveled("dragon_knight_elder_dragon_form", 1)],
            vec![
                leveled("dragon_knight_wyrms_wrath", 1),
                inferred(MatchEventKind::ItemAdded {
                    item: "item_boots".into(),
                    placement: ItemPlacement::Carried,
                }),
            ],
            vec![
                leveled("dragon_knight_breathe_fire", 4),
                confirmed(MatchEventKind::HeroDied),
            ],
            // O item chegou ao inventário vindo do stash, entre eventos que a
            // fixture não inclui; aqui ele aparece pela primeira vez.
            vec![inferred(MatchEventKind::ItemAdded {
                item: "item_point_booster".into(),
                placement: ItemPlacement::Carried,
            })],
        ]
    );
}

#[test]
fn synthetic_post_game_reports_victory_for_winning_team() {
    let events = events_per_payload("synthetic_post_game.jsonl");

    assert_eq!(
        events,
        vec![
            vec![],
            vec![confirmed(MatchEventKind::MatchEnded {
                result: MatchResult::Victory,
            })],
        ]
    );
}
