//! Testes unitários de `src/domain/game.rs`.

use super::*;

#[test]
fn converts_every_observed_game_state() {
    let cases = [
        (
            "DOTA_GAMERULES_STATE_HERO_SELECTION",
            GamePhase::HeroSelection,
        ),
        (
            "DOTA_GAMERULES_STATE_STRATEGY_TIME",
            GamePhase::StrategyTime,
        ),
        (
            "DOTA_GAMERULES_STATE_TEAM_SHOWCASE",
            GamePhase::TeamShowcase,
        ),
        (
            "DOTA_GAMERULES_STATE_WAIT_FOR_MAP_TO_LOAD",
            GamePhase::WaitForMapToLoad,
        ),
        ("DOTA_GAMERULES_STATE_PRE_GAME", GamePhase::PreGame),
        (
            "DOTA_GAMERULES_STATE_GAME_IN_PROGRESS",
            GamePhase::InProgress,
        ),
        ("DOTA_GAMERULES_STATE_POST_GAME", GamePhase::PostGame),
    ];
    for (game_state, expected) in cases {
        assert_eq!(GamePhase::from_gsi(game_state), expected, "{game_state}");
    }
}

#[test]
fn keeps_unknown_game_state_as_received() {
    assert_eq!(
        GamePhase::from_gsi("DOTA_GAMERULES_STATE_DISCONNECT"),
        GamePhase::Other("DOTA_GAMERULES_STATE_DISCONNECT".into())
    );
}

#[test]
fn orders_match_phases_and_leaves_unknown_out() {
    assert!(GamePhase::HeroSelection.sequence_position() < GamePhase::PreGame.sequence_position());
    assert!(GamePhase::PreGame.sequence_position() < GamePhase::InProgress.sequence_position());
    assert_eq!(GamePhase::Other("X".into()).sequence_position(), None);
}

#[test]
fn reads_only_radiant_and_dire_as_teams() {
    assert_eq!(Team::from_gsi("radiant"), Some(Team::Radiant));
    assert_eq!(Team::from_gsi("dire"), Some(Team::Dire));
    assert_eq!(Team::from_gsi("none"), None);
}
