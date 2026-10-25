//! Testes unitários de `src/gsi/normalize.rs`.

use super::*;
use crate::test_support::{
    DRAGON_KNIGHT_MATCH, DRAGON_KNIGHT_MATCH_END, ZEUS_MATCH, fixture_payloads,
};
use serde_json::json;

fn zeus() -> Vec<GameSnapshot> {
    fixture_payloads(ZEUS_MATCH)
        .iter()
        .map(game_snapshot)
        .collect()
}

fn item(slot: &str, name: &str) -> CarriedItem {
    CarriedItem {
        slot: slot.into(),
        name: name.into(),
    }
}

fn ability(name: &str, level: u32, ultimate: bool) -> AbilityState {
    AbilityState {
        name: name.into(),
        level,
        ultimate,
    }
}

#[test]
fn menu_event_without_map_has_no_match_data() {
    assert_eq!(zeus()[0], GameSnapshot::menu());
}

#[test]
fn hero_with_zero_id_before_pick_is_not_picked() {
    let snapshot = &zeus()[1];
    assert_eq!(snapshot.phase, Some(GamePhase::HeroSelection));
    assert_eq!(snapshot.hero, HeroPick::NotPicked);
    assert_eq!(snapshot.team, Some(Team::Dire));
    assert_eq!(snapshot.stats.gold, Some(600));
    assert_eq!(snapshot.abilities, None);
    assert_eq!(snapshot.inventory, None);
}

#[test]
fn picked_hero_has_internal_name_and_id() {
    let snapshot = &zeus()[2];
    assert_eq!(snapshot.phase, Some(GamePhase::StrategyTime));
    assert_eq!(
        snapshot.hero,
        HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: Some(22),
        }
    );
    assert_eq!(snapshot.hero_level, None);
}

#[test]
fn empty_items_block_during_strategy_is_unknown_inventory_not_empty() {
    let snapshot = &zeus()[4];
    assert_eq!(snapshot.stats.gold, Some(60));
    assert_eq!(snapshot.inventory, None);
}

#[test]
fn pre_game_brings_inventory_and_abilities_in_slot_order() {
    let snapshot = &zeus()[7];
    assert_eq!(snapshot.phase, Some(GamePhase::PreGame));
    assert_eq!(
        snapshot.inventory,
        Some(vec![
            item("slot0", "item_circlet"),
            item("slot1", "item_circlet"),
            item("slot2", "item_mantle"),
            item("slot3", "item_tango"),
            item("teleport0", "item_tpscroll"),
        ])
    );
    assert_eq!(
        snapshot.abilities,
        Some(vec![
            ability("zuus_arc_lightning", 0, false),
            ability("zuus_lightning_bolt", 0, false),
            ability("zuus_heavenly_jump", 0, false),
            ability("zuus_thundergods_wrath", 0, true),
        ])
    );
    assert_eq!(snapshot.hero_level, Some(1));
    assert_eq!(snapshot.hero_alive, Some(true));
}

#[test]
fn reads_player_stats_and_death_from_match_in_progress() {
    let snapshot = game_snapshot(&fixture_payloads(DRAGON_KNIGHT_MATCH)[4]);
    assert_eq!(snapshot.phase, Some(GamePhase::InProgress));
    assert_eq!(snapshot.hero_alive, Some(false));
    assert_eq!(snapshot.hero_level, Some(8));
    assert_eq!(
        snapshot.stats,
        PlayerStats {
            kills: Some(0),
            deaths: Some(1),
            assists: Some(0),
            last_hits: Some(28),
            denies: Some(2),
            gold: Some(76),
            gpm: Some(223),
            xpm: Some(399),
        }
    );
    assert_eq!(snapshot.winner, None);
}

#[test]
fn post_game_reads_winning_team() {
    let snapshot = game_snapshot(&fixture_payloads(DRAGON_KNIGHT_MATCH_END)[2]);
    assert_eq!(snapshot.phase, Some(GamePhase::PostGame));
    assert_eq!(snapshot.winner, Some(Team::Radiant));
    assert_eq!(snapshot.team, Some(Team::Radiant));
    assert_eq!(snapshot.clock_time, Some(1429));
    assert_eq!(snapshot.match_id, None);
}

#[test]
fn match_id_arrives_in_a_later_post_game_event() {
    let snapshot = game_snapshot(&fixture_payloads(DRAGON_KNIGHT_MATCH_END)[3]);
    assert_eq!(snapshot.phase, Some(GamePhase::PostGame));
    assert_eq!(
        snapshot.match_id.as_ref().map(MatchId::as_str),
        Some("1234567890")
    );
}

#[test]
fn ignores_fields_with_unexpected_types_and_keeps_the_rest() {
    let snapshot = game_snapshot(&json!({
        "map": { "game_state": 5, "win_team": ["radiant"] },
        "hero": { "name": "npc_dota_hero_zuus", "id": "22", "level": -3, "alive": "yes" },
        "player": { "team_name": "radiant", "kills": "dez", "deaths": 2 },
        "abilities": { "ability0": "zuus_arc_lightning", "abilityX": { "name": "x" } },
        "items": { "slot0": { "name": 7 }, "slot1": { "name": "item_tango" } },
    }));
    assert_eq!(snapshot.phase, None);
    assert_eq!(snapshot.winner, None);
    assert_eq!(
        snapshot.hero,
        HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: None,
        }
    );
    assert_eq!(snapshot.hero_level, None);
    assert_eq!(snapshot.hero_alive, None);
    assert_eq!(snapshot.team, Some(Team::Radiant));
    assert_eq!(snapshot.stats.kills, None);
    assert_eq!(snapshot.stats.deaths, Some(2));
    assert_eq!(snapshot.abilities, Some(Vec::new()));
    assert_eq!(snapshot.inventory, Some(vec![item("slot1", "item_tango")]));
}

#[test]
fn unknown_game_state_is_kept() {
    let snapshot = game_snapshot(&json!({
        "map": { "game_state": "DOTA_GAMERULES_STATE_CUSTOM_GAME_SETUP" }
    }));
    assert_eq!(
        snapshot.phase,
        Some(GamePhase::Other(
            "DOTA_GAMERULES_STATE_CUSTOM_GAME_SETUP".into()
        ))
    );
}

#[test]
fn payload_that_is_not_an_object_is_like_menu() {
    assert_eq!(game_snapshot(&json!([1, 2, 3])), GameSnapshot::menu());
}

#[test]
fn reads_negative_clock_before_match_starts() {
    assert_eq!(zeus()[1].clock_time, Some(-74));
}

#[test]
fn zero_match_id_during_bot_match_is_no_id() {
    assert!(zeus().iter().all(|snapshot| snapshot.match_id.is_none()));
}

#[test]
fn reads_real_match_id_and_clock_from_post_game() {
    let snapshot = game_snapshot(&json!({
        "map": {
            "game_state": "DOTA_GAMERULES_STATE_POST_GAME",
            "clock_time": 1429,
            "matchid": "9060449998",
        }
    }));
    assert_eq!(snapshot.clock_time, Some(1429));
    assert_eq!(
        snapshot.match_id.as_ref().map(MatchId::as_str),
        Some("9060449998")
    );
}

#[test]
fn clock_and_match_id_with_unexpected_types_are_no_data() {
    let snapshot = game_snapshot(&json!({
        "map": {
            "game_state": "DOTA_GAMERULES_STATE_GAME_IN_PROGRESS",
            "clock_time": "300",
            "matchid": 9060449998_u64,
        }
    }));
    assert_eq!(snapshot.phase, Some(GamePhase::InProgress));
    assert_eq!(snapshot.clock_time, None);
    assert_eq!(snapshot.match_id, None);
}

#[test]
fn clock_outside_the_supported_range_is_no_data() {
    let snapshot = game_snapshot(&json!({ "map": { "clock_time": i64::MAX } }));
    assert_eq!(snapshot.clock_time, None);
}
