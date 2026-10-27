//! Testes unitários de `src/presentation/pages/hero.rs` (regras de exibição,
//! sem widgets).

use super::*;
use crate::domain::game::{CarriedItem, MatchId};
use crate::domain::game::{GameMode, GamePhase, PlayerStats};
use crate::domain::hero_catalog::{AbilityInfo, AttackType};

fn zeus_info() -> HeroInfo {
    HeroInfo {
        image_path: None,
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![HeroRole::Nuker, HeroRole::Carry],
        abilities: vec![
            AbilityInfo {
                image_path: None,
                name: "zuus_arc_lightning".into(),
                display_name: "Arc Lightning".into(),
            },
            AbilityInfo {
                image_path: None,
                name: "zuus_static_field".into(),
                display_name: "Static Field".into(),
            },
        ],
    }
}

fn catalog() -> HeroCatalog {
    HeroCatalog::new(vec![zeus_info()])
}

fn picked(name: &str, id: Option<u32>) -> GameSnapshot {
    GameSnapshot {
        phase: Some(GamePhase::StrategyTime),
        hero: HeroPick::Picked {
            name: name.into(),
            id,
        },
        ..GameSnapshot::menu()
    }
}

#[test]
fn picked_hero_is_found_by_internal_name() {
    let catalog = catalog();
    assert_eq!(
        picked_hero(&picked("npc_dota_hero_zuus", None), &catalog),
        Some(&zeus_info())
    );
}

#[test]
fn picked_hero_falls_back_to_id_when_name_is_unknown() {
    let catalog = catalog();
    assert_eq!(
        picked_hero(&picked("npc_dota_hero_renomeado", Some(22)), &catalog),
        Some(&zeus_info())
    );
}

#[test]
fn hero_not_in_catalog_or_not_picked_has_no_static_data() {
    let catalog = catalog();
    assert_eq!(
        picked_hero(&picked("npc_dota_hero_novo", None), &catalog),
        None
    );
    assert_eq!(picked_hero(&GameSnapshot::menu(), &catalog), None);
}

#[test]
fn static_data_labels_are_in_portuguese() {
    assert_eq!(attribute_label(&PrimaryAttribute::Universal), "Universal");
    assert_eq!(attribute_label(&PrimaryAttribute::Strength), "Força");
    assert_eq!(role_label(&HeroRole::Support), "Suporte");
    assert_eq!(role_label(&HeroRole::Other("Jungler".into())), "Jungler");
}

fn summary() -> MatchSummary {
    MatchSummary {
        hero: HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: Some(22),
        },
        result: MatchResult::Victory,
        duration_secs: Some(1429),
        hero_level: Some(17),
        stats: PlayerStats {
            kills: Some(12),
            deaths: Some(0),
            assists: Some(10),
            last_hits: Some(113),
            denies: Some(6),
            gold: None,
            gpm: Some(520),
            xpm: Some(697),
        },
        final_items: vec![CarriedItem {
            slot: "slot0".into(),
            name: "item_satanic".into(),
        }],
        abilities: vec![],
        match_id: MatchId::from_gsi("9060449998"),
    }
}

#[test]
fn last_match_state_reuses_hero_stats_items_and_abilities() {
    let snapshot = last_match_snapshot(&summary());
    assert_eq!(snapshot.phase, Some(GamePhase::PostGame));
    assert_eq!(snapshot.hero, summary().hero);
    assert_eq!(snapshot.stats, summary().stats);
    assert_eq!(snapshot.inventory, Some(summary().final_items));
    assert_eq!(snapshot.abilities, Some(vec![]));
}

#[test]
fn result_lines_show_result_duration_and_match_id() {
    assert_eq!(
        result_lines(&summary()),
        vec![
            "Resultado: vitória".to_owned(),
            "Duração: 23:49".to_owned(),
            "ID da partida: 9060449998".to_owned(),
        ]
    );
}

#[test]
fn result_lines_skip_missing_duration_and_match_id() {
    let unknown = MatchSummary {
        result: MatchResult::Unknown,
        duration_secs: None,
        match_id: None,
        ..summary()
    };
    assert_eq!(
        result_lines(&unknown),
        vec!["Resultado não informado pelo Dota".to_owned()]
    );
}

fn awaiting(match_number: u64, preselected: GameMode) -> MatchStatus {
    MatchStatus {
        match_number,
        mode: ModeState::Awaiting { preselected },
        ..MatchStatus::default()
    }
}

fn confirmed(mode: GameMode, origin: ModeOrigin) -> MatchStatus {
    MatchStatus {
        match_number: 1,
        mode: ModeState::Confirmed { mode, origin },
        ..MatchStatus::default()
    }
}

#[test]
fn mode_labels_are_in_portuguese_without_repetition() {
    let labels: Vec<String> = GameMode::ALL.into_iter().map(mode_label).collect();
    assert_eq!(labels, ["Escolha Livre", "Turbo", "Contra bots"]);
}

#[test]
fn found_match_opens_the_mode_question_once() {
    let status = awaiting(1, GameMode::Turbo);

    assert_eq!(mode_dialog_request(&status, 0), Some((1, GameMode::Turbo)));
    assert_eq!(mode_dialog_request(&status, 1), None);
}

#[test]
fn mode_question_does_not_open_without_a_waiting_match() {
    assert_eq!(mode_dialog_request(&MatchStatus::default(), 0), None);
    assert_eq!(
        mode_dialog_request(&confirmed(GameMode::Turbo, ModeOrigin::Chosen), 0),
        None
    );
}

#[test]
fn confirm_button_records_the_selected_mode_as_chosen() {
    assert_eq!(
        mode_dialog_outcome(true, GameMode::AllPick, GameMode::Turbo),
        (GameMode::Turbo, ModeOrigin::Chosen)
    );
}

#[test]
fn closing_the_question_keeps_the_preselected_mode_as_default() {
    assert_eq!(
        mode_dialog_outcome(false, GameMode::AllPick, GameMode::Turbo),
        (GameMode::AllPick, ModeOrigin::Default),
        "mudanças não confirmadas são descartadas"
    );
}
