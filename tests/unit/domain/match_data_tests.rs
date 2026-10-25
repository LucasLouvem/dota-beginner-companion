//! Testes unitários de `src/domain/match_data.rs`.

use super::*;

fn player(patch_id: Option<u32>) -> MatchPlayer {
    MatchPlayer {
        hero_id: 22,
        won: Some(true),
        patch_id,
        purchase_log: None,
        final_items: Some(vec!["item_satanic".into()]),
        ability_upgrades: None,
    }
}

fn details(league_id: Option<u32>, patch_id: Option<u32>) -> MatchDetails {
    MatchDetails {
        match_id: 1,
        league_id,
        game_mode: Some(22),
        patch_id,
        parsed: false,
        start_time: None,
        players: vec![player(None)],
    }
}

#[test]
fn league_match_needs_a_non_zero_league_id() {
    assert!(details(Some(18631), None).is_league_match());
    assert!(!details(Some(0), None).is_league_match());
    assert!(!details(None, None).is_league_match());
}

#[test]
fn player_patch_comes_first_then_the_match_patch() {
    let match_details = details(None, Some(59));
    assert_eq!(match_details.patch_of(&player(Some(60))), Some(60));
    assert_eq!(match_details.patch_of(&player(None)), Some(59));
    assert_eq!(details(None, None).patch_of(&player(None)), None);
}

#[test]
fn build_input_needs_result_and_final_items() {
    assert!(player(None).build_input().is_some());
    let without_result = MatchPlayer {
        won: None,
        ..player(None)
    };
    assert_eq!(without_result.build_input(), None);
    let without_items = MatchPlayer {
        final_items: None,
        ..player(None)
    };
    assert_eq!(without_items.build_input(), None);
}

#[test]
fn finds_the_player_with_the_hero() {
    assert_eq!(
        details(None, None)
            .player_with_hero(22)
            .map(|player| player.hero_id),
        Some(22)
    );
    assert_eq!(details(None, None).player_with_hero(49), None);
}
