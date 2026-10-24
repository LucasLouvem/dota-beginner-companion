//! Testes unitários de `src/domain/match_events.rs`, com estados montados à
//! mão. As sequências reais das fixtures ficam em `tests/integration/match_flow.rs`.

use super::*;
use crate::domain::game::{AbilityState, CarriedItem, Team};

fn in_phase(phase: GamePhase) -> GameSnapshot {
    GameSnapshot {
        phase: Some(phase),
        ..GameSnapshot::menu()
    }
}

fn with_items(phase: GamePhase, items: &[(&str, &str)]) -> GameSnapshot {
    GameSnapshot {
        inventory: Some(
            items
                .iter()
                .map(|(slot, name)| CarriedItem {
                    slot: (*slot).into(),
                    name: (*name).into(),
                })
                .collect(),
        ),
        ..in_phase(phase)
    }
}

fn observe_all(snapshots: Vec<GameSnapshot>) -> Vec<MatchEvent> {
    let mut tracker = MatchTracker::default();
    snapshots
        .into_iter()
        .flat_map(|snapshot| tracker.observe(snapshot))
        .collect()
}

fn kinds(events: &[MatchEvent]) -> Vec<MatchEventKind> {
    events.iter().map(|event| event.kind.clone()).collect()
}

#[test]
fn hero_selection_after_menu_is_an_inferred_match_found() {
    let events = observe_all(vec![
        GameSnapshot::menu(),
        in_phase(GamePhase::HeroSelection),
    ]);
    assert_eq!(
        events,
        vec![MatchEvent {
            kind: MatchEventKind::MatchFound,
            provenance: Provenance::Inferred,
        }]
    );
}

#[test]
fn going_back_to_hero_selection_in_same_match_finds_nothing() {
    let events = observe_all(vec![
        in_phase(GamePhase::PreGame),
        in_phase(GamePhase::HeroSelection),
    ]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn match_start_is_announced_once_even_when_phase_goes_back() {
    let events = observe_all(vec![
        in_phase(GamePhase::HeroSelection),
        in_phase(GamePhase::PreGame),
        in_phase(GamePhase::InProgress),
        in_phase(GamePhase::PreGame),
        GameSnapshot::menu(),
        in_phase(GamePhase::InProgress),
    ]);
    assert_eq!(
        kinds(&events),
        vec![MatchEventKind::MatchFound, MatchEventKind::MatchStarted]
    );
}

#[test]
fn first_event_already_in_progress_is_not_a_match_start() {
    let events = observe_all(vec![
        in_phase(GamePhase::InProgress),
        in_phase(GamePhase::InProgress),
    ]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn incomplete_event_without_map_does_not_create_events() {
    let events = observe_all(vec![in_phase(GamePhase::InProgress), GameSnapshot::menu()]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn hero_pick_is_confirmed_with_internal_name() {
    let picked = GameSnapshot {
        hero: HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: Some(22),
        },
        ..in_phase(GamePhase::StrategyTime)
    };
    let events = observe_all(vec![in_phase(GamePhase::HeroSelection), picked]);
    assert_eq!(
        events[1],
        MatchEvent {
            kind: MatchEventKind::HeroPicked {
                hero: "npc_dota_hero_zuus".into(),
            },
            provenance: Provenance::Confirmed,
        }
    );
}

#[test]
fn post_game_reports_victory_when_own_team_wins() {
    let ended = GameSnapshot {
        team: Some(Team::Radiant),
        winner: Some(Team::Radiant),
        ..in_phase(GamePhase::PostGame)
    };
    let events = observe_all(vec![in_phase(GamePhase::InProgress), ended.clone(), ended]);
    assert_eq!(
        events,
        vec![MatchEvent {
            kind: MatchEventKind::MatchEnded {
                result: MatchResult::Victory,
            },
            provenance: Provenance::Confirmed,
        }]
    );
}

#[test]
fn post_game_reports_defeat_when_other_team_wins() {
    let ended = GameSnapshot {
        team: Some(Team::Radiant),
        winner: Some(Team::Dire),
        ..in_phase(GamePhase::PostGame)
    };
    let events = observe_all(vec![ended]);
    assert_eq!(
        kinds(&events),
        vec![MatchEventKind::MatchEnded {
            result: MatchResult::Defeat
        }]
    );
}

#[test]
fn post_game_without_winner_has_unknown_result() {
    let events = observe_all(vec![in_phase(GamePhase::PostGame)]);
    assert_eq!(
        kinds(&events),
        vec![MatchEventKind::MatchEnded {
            result: MatchResult::Unknown
        }]
    );
}

#[test]
fn new_match_after_post_game_starts_tracking_again() {
    let events = observe_all(vec![
        in_phase(GamePhase::PreGame),
        in_phase(GamePhase::InProgress),
        in_phase(GamePhase::PostGame),
        in_phase(GamePhase::HeroSelection),
        in_phase(GamePhase::PreGame),
        in_phase(GamePhase::InProgress),
    ]);
    assert_eq!(
        kinds(&events),
        vec![
            MatchEventKind::MatchStarted,
            MatchEventKind::MatchEnded {
                result: MatchResult::Unknown
            },
            MatchEventKind::MatchFound,
            MatchEventKind::MatchStarted,
        ]
    );
}

#[test]
fn moving_item_between_slots_is_not_an_inventory_change() {
    let events = observe_all(vec![
        with_items(GamePhase::InProgress, &[("stash0", "item_point_booster")]),
        with_items(GamePhase::InProgress, &[("slot5", "item_point_booster")]),
    ]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn inventory_changes_are_inferred_once_per_unit() {
    let events = observe_all(vec![
        with_items(
            GamePhase::InProgress,
            &[("slot0", "item_tango"), ("slot1", "item_circlet")],
        ),
        with_items(
            GamePhase::InProgress,
            &[
                ("slot1", "item_circlet"),
                ("slot2", "item_branches"),
                ("slot3", "item_branches"),
            ],
        ),
    ]);
    assert_eq!(
        events,
        vec![
            MatchEvent {
                kind: MatchEventKind::ItemAdded {
                    item: "item_branches".into(),
                },
                provenance: Provenance::Inferred,
            },
            MatchEvent {
                kind: MatchEventKind::ItemAdded {
                    item: "item_branches".into(),
                },
                provenance: Provenance::Inferred,
            },
            MatchEvent {
                kind: MatchEventKind::ItemRemoved {
                    item: "item_tango".into(),
                },
                provenance: Provenance::Inferred,
            },
        ]
    );
}

#[test]
fn first_known_inventory_does_not_report_every_item_as_added() {
    let events = observe_all(vec![
        in_phase(GamePhase::StrategyTime),
        with_items(GamePhase::PreGame, &[("slot0", "item_tango")]),
    ]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn ability_levels_are_not_compared_across_different_heroes() {
    let hero = |name: &str, level| GameSnapshot {
        hero: HeroPick::Picked {
            name: name.into(),
            id: None,
        },
        abilities: Some(vec![AbilityState {
            name: "shared_ability".into(),
            level,
            ultimate: false,
        }]),
        ..in_phase(GamePhase::InProgress)
    };
    let events = observe_all(vec![hero("npc_a", 1), hero("npc_b", 3)]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn manual_match_found_is_marked_as_manual() {
    assert_eq!(
        MatchEvent::manual_match_found(),
        MatchEvent {
            kind: MatchEventKind::MatchFound,
            provenance: Provenance::Manual,
        }
    );
}
