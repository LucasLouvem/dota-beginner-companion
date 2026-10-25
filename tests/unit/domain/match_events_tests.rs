//! Testes unitários de `src/domain/match_events.rs`, com estados montados à
//! mão. As sequências reais das fixtures ficam em `tests/integration/match_flow.rs`.

use super::*;
use crate::domain::game::{AbilityState, CarriedItem, MatchId, PlayerStats, Team};
use crate::domain::item_catalog::{ItemCatalog, ItemInfo};

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
                    placement: ItemPlacement::Carried,
                },
                provenance: Provenance::Inferred,
            },
            MatchEvent {
                kind: MatchEventKind::ItemAdded {
                    item: "item_branches".into(),
                    placement: ItemPlacement::Carried,
                },
                provenance: Provenance::Inferred,
            },
            MatchEvent {
                kind: MatchEventKind::ItemRemoved {
                    item: "item_tango".into(),
                    placement: ItemPlacement::Carried,
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

fn recipes() -> Arc<ItemCatalog> {
    let item = |name: &str, components: &[&str]| ItemInfo {
        name: name.into(),
        display_name: name.into(),
        components: components
            .iter()
            .map(|component| (*component).into())
            .collect(),
    };
    Arc::new(ItemCatalog::new(vec![
        item(
            "item_satanic",
            &["item_reaver", "item_claymore", "item_lifesteal"],
        ),
        item(
            "item_power_treads",
            &["item_boots", "item_gloves", "item_belt_of_strength"],
        ),
        item("item_tango", &[]),
    ]))
}

fn observe_with_recipes(snapshots: Vec<GameSnapshot>) -> Vec<MatchEvent> {
    let mut tracker = MatchTracker::new(recipes());
    snapshots
        .into_iter()
        .flat_map(|snapshot| tracker.observe(snapshot))
        .collect()
}

#[test]
fn teleport_scroll_used_and_bought_again_creates_no_event() {
    let events = observe_all(vec![
        with_items(GamePhase::InProgress, &[("teleport0", "item_tpscroll")]),
        with_items(GamePhase::InProgress, &[]),
        with_items(GamePhase::InProgress, &[("teleport0", "item_tpscroll")]),
    ]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn item_in_neutral_slot_is_marked_as_neutral() {
    let events = observe_all(vec![
        with_items(GamePhase::InProgress, &[("neutral0", "item_weighted_dice")]),
        with_items(
            GamePhase::InProgress,
            &[("neutral0", "item_searing_signet")],
        ),
    ]);
    assert_eq!(
        kinds(&events),
        vec![
            MatchEventKind::ItemAdded {
                item: "item_searing_signet".into(),
                placement: ItemPlacement::Neutral,
            },
            MatchEventKind::ItemRemoved {
                item: "item_weighted_dice".into(),
                placement: ItemPlacement::Neutral,
            },
        ]
    );
}

#[test]
fn assembled_item_replaces_its_components_that_left() {
    let events = observe_with_recipes(vec![
        with_items(
            GamePhase::InProgress,
            &[
                ("slot0", "item_reaver"),
                ("slot1", "item_claymore"),
                ("slot2", "item_lifesteal"),
                ("slot3", "item_soul_ring"),
            ],
        ),
        with_items(
            GamePhase::InProgress,
            &[("slot0", "item_satanic"), ("slot3", "item_soul_ring")],
        ),
    ]);
    assert_eq!(
        events,
        vec![MatchEvent {
            kind: MatchEventKind::ItemAssembled {
                item: "item_satanic".into(),
                components: vec![
                    "item_reaver".into(),
                    "item_claymore".into(),
                    "item_lifesteal".into(),
                ],
            },
            provenance: Provenance::Inferred,
        }]
    );
}

#[test]
fn partial_assembly_lists_only_components_that_left_and_keeps_other_removals() {
    let events = observe_with_recipes(vec![
        with_items(
            GamePhase::InProgress,
            &[("slot0", "item_boots"), ("slot1", "item_tango")],
        ),
        with_items(GamePhase::InProgress, &[("slot0", "item_power_treads")]),
    ]);
    assert_eq!(
        kinds(&events),
        vec![
            MatchEventKind::ItemAssembled {
                item: "item_power_treads".into(),
                components: vec!["item_boots".into()],
            },
            MatchEventKind::ItemRemoved {
                item: "item_tango".into(),
                placement: ItemPlacement::Carried,
            },
        ]
    );
}

#[test]
fn item_without_recipe_entering_while_another_leaves_is_not_an_assembly() {
    let events = observe_with_recipes(vec![
        with_items(GamePhase::InProgress, &[("slot0", "item_claymore")]),
        with_items(GamePhase::InProgress, &[("slot0", "item_tango")]),
    ]);
    assert_eq!(
        kinds(&events),
        vec![
            MatchEventKind::ItemAdded {
                item: "item_tango".into(),
                placement: ItemPlacement::Carried,
            },
            MatchEventKind::ItemRemoved {
                item: "item_claymore".into(),
                placement: ItemPlacement::Carried,
            },
        ]
    );
}

#[test]
fn tracker_without_recipes_reports_assembly_as_plain_changes() {
    let events = observe_all(vec![
        with_items(GamePhase::InProgress, &[("slot0", "item_boots")]),
        with_items(GamePhase::InProgress, &[("slot0", "item_power_treads")]),
    ]);
    assert_eq!(
        kinds(&events),
        vec![
            MatchEventKind::ItemAdded {
                item: "item_power_treads".into(),
                placement: ItemPlacement::Carried,
            },
            MatchEventKind::ItemRemoved {
                item: "item_boots".into(),
                placement: ItemPlacement::Carried,
            },
        ]
    );
}

#[test]
fn summary_of_post_game_has_result_duration_stats_items_and_match_id() {
    let post_game = GameSnapshot {
        clock_time: Some(1429),
        match_id: MatchId::from_gsi("9060449998"),
        hero: HeroPick::Picked {
            name: "npc_dota_hero_dragon_knight".into(),
            id: Some(49),
        },
        hero_level: Some(17),
        team: Some(Team::Radiant),
        winner: Some(Team::Radiant),
        stats: PlayerStats {
            kills: Some(12),
            deaths: Some(0),
            assists: Some(10),
            last_hits: Some(113),
            denies: Some(6),
            gold: Some(1500),
            gpm: Some(520),
            xpm: Some(697),
        },
        inventory: Some(vec![CarriedItem {
            slot: "slot0".into(),
            name: "item_satanic".into(),
        }]),
        abilities: Some(vec![AbilityState {
            name: "dragon_knight_breathe_fire".into(),
            level: 4,
            ultimate: false,
        }]),
        ..in_phase(GamePhase::PostGame)
    };

    let summary = MatchSummary::from_post_game(&post_game);
    assert_eq!(summary.result, MatchResult::Victory);
    assert_eq!(summary.duration_secs, Some(1429));
    assert_eq!(summary.hero_level, Some(17));
    assert_eq!(summary.stats, post_game.stats);
    assert_eq!(summary.final_items.len(), 1);
    assert_eq!(summary.abilities.len(), 1);
    assert_eq!(
        summary.match_id.as_ref().map(MatchId::as_str),
        Some("9060449998")
    );
}

#[test]
fn summary_without_data_from_the_dota_keeps_fields_empty() {
    let summary = MatchSummary::from_post_game(&in_phase(GamePhase::PostGame));
    assert_eq!(summary.hero, HeroPick::NotPicked);
    assert_eq!(summary.result, MatchResult::Unknown);
    assert_eq!(summary.duration_secs, None);
    assert_eq!(summary.stats, PlayerStats::default());
    assert!(summary.final_items.is_empty());
    assert!(summary.abilities.is_empty());
    assert_eq!(summary.match_id, None);
}

#[test]
fn negative_clock_is_not_a_duration() {
    let snapshot = GameSnapshot {
        clock_time: Some(-30),
        ..in_phase(GamePhase::PostGame)
    };
    assert_eq!(MatchSummary::from_post_game(&snapshot).duration_secs, None);
}
