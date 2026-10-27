//! Testes unitários de `src/presentation/pages/build_cards.rs` (conteúdo dos
//! cards da página Herói, sem widgets).

use super::*;
use crate::application::state::ModeState;
use crate::application::use_cases::fallback::DataOrigin;
use crate::domain::game::{GameMode, GamePhase, GameSnapshot, HeroPick, PlayerStats};
use crate::domain::hero_catalog::{AttackType, HeroRole, PrimaryAttribute};
use crate::domain::match_events::{MatchResult, MatchSummary};
use crate::test_support::{pro_builds, recommendations};
use std::sync::Arc;

fn catalog() -> HeroCatalog {
    HeroCatalog::new(vec![
        HeroInfo {
            image_path: None,
            id: 22,
            name: "npc_dota_hero_zuus".into(),
            display_name: "Zeus".into(),
            primary_attribute: PrimaryAttribute::Intelligence,
            attack_type: AttackType::Ranged,
            roles: vec![HeroRole::Nuker],
            abilities: vec![],
        },
        HeroInfo {
            image_path: None,
            id: 37,
            name: "npc_dota_hero_warlock".into(),
            display_name: "Warlock".into(),
            primary_attribute: PrimaryAttribute::Intelligence,
            attack_type: AttackType::Ranged,
            roles: vec![],
            abilities: vec![],
        },
    ])
}

fn picked(name: &str, id: u32) -> HeroPick {
    HeroPick::Picked {
        name: name.into(),
        id: Some(id),
    }
}

fn game_with(hero: HeroPick) -> GameSnapshot {
    GameSnapshot {
        hero,
        ..GameSnapshot::menu()
    }
}

fn summary() -> MatchSummary {
    MatchSummary {
        hero: picked("npc_dota_hero_warlock", 37),
        result: MatchResult::Victory,
        duration_secs: Some(1200),
        hero_level: Some(10),
        stats: PlayerStats::default(),
        final_items: vec![],
        abilities: vec![],
        match_id: None,
    }
}

fn ready(mode: GameMode, origin: Option<DataOrigin>) -> RecommendationStatus {
    RecommendationStatus::Ready(Arc::new(recommendations(
        mode,
        pro_builds(vec![], 7, origin),
        vec![],
    )))
}

#[test]
fn header_uses_the_current_hero_first() {
    let status = MatchStatus {
        game: Some(game_with(picked("npc_dota_hero_zuus", 22))),
        last_match: Some(summary()),
        ..MatchStatus::default()
    };
    let header = hero_header(&status, &ready(GameMode::AllPick, None), &catalog());
    assert_eq!(header.name, "Zeus");
    assert_eq!(header.chips.len(), 2, "atributo e uma função");
}

#[test]
fn header_falls_back_to_the_last_match_hero() {
    let status = MatchStatus {
        game: Some(GameSnapshot::menu()),
        last_match: Some(summary()),
        ..MatchStatus::default()
    };
    let header = hero_header(&status, &RecommendationStatus::Idle, &catalog());
    assert_eq!(header.name, "Warlock");
    assert!(header.last_match.starts_with("Última partida"));
}

#[test]
fn header_falls_back_to_the_hero_of_the_selected_build() {
    // A consulta de teste é do herói 22 (Zeus).
    let header = hero_header(
        &MatchStatus::default(),
        &ready(GameMode::AllPick, Some(DataOrigin::Live)),
        &catalog(),
    );
    assert_eq!(header.name, "Zeus");
    assert!(header.source.starts_with("Patch "));
    assert!(header.last_match.is_empty());
}

#[test]
fn header_without_any_hero_says_so() {
    let header = hero_header(
        &MatchStatus::default(),
        &RecommendationStatus::Idle,
        &catalog(),
    );
    assert_eq!(header.name, "Nenhum herói escolhido");
    assert!(header.chips.is_empty());
    assert!(header.source.is_empty());
}

#[test]
fn hero_chips_hold_only_the_fixed_data_of_the_hero() {
    let status = MatchStatus {
        game: Some(game_with(picked("npc_dota_hero_zuus", 22))),
        mode: ModeState::NotAsked,
        ..MatchStatus::default()
    };
    let header = hero_header(&status, &RecommendationStatus::Idle, &catalog());
    assert_eq!(
        header.chips,
        ["Inteligência".to_owned(), role_label(&HeroRole::Nuker)]
    );
    assert!(
        header.chips.iter().all(|chip| !chip.contains("Modo")),
        "o modo não é um atributo"
    );
}

fn status_in(phase: Option<GamePhase>) -> MatchStatus {
    MatchStatus {
        game: Some(GameSnapshot {
            phase,
            ..GameSnapshot::menu()
        }),
        ..MatchStatus::default()
    }
}

#[test]
fn the_player_is_in_a_match_from_loading_until_the_game_ends() {
    for phase in [
        GamePhase::WaitForPlayersToLoad,
        GamePhase::HeroSelection,
        GamePhase::StrategyTime,
        GamePhase::TeamShowcase,
        GamePhase::WaitForMapToLoad,
        GamePhase::PreGame,
        GamePhase::InProgress,
    ] {
        assert!(in_match(&status_in(Some(phase.clone()))), "{phase:?}");
    }
}

#[test]
fn menu_post_game_unknown_phases_and_no_game_are_not_a_match() {
    assert!(!in_match(&status_in(None)), "menu, sem o bloco map");
    assert!(!in_match(&status_in(Some(GamePhase::PostGame))));
    assert!(!in_match(&status_in(Some(GamePhase::Other("x".into())))));
    assert!(!in_match(&MatchStatus::default()));
}

#[test]
fn the_header_says_whether_the_player_is_in_a_match() {
    let in_game = status_in(Some(GamePhase::InProgress));
    let header = hero_header(&in_game, &RecommendationStatus::Idle, &catalog());
    assert!(header.in_match);
    let header = hero_header(
        &MatchStatus::default(),
        &RecommendationStatus::Idle,
        &catalog(),
    );
    assert!(!header.in_match);
}
