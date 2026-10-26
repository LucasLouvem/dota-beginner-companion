//! Testes unitários de `src/presentation/pages/abilities.rs`.

use super::*;
use crate::application::use_cases::fallback::DataOrigin;
use crate::domain::builds::PlayerBuild;
use crate::domain::hero_catalog::{AbilityInfo, AttackType, HeroInfo, PrimaryAttribute};
use crate::test_support::{pro_builds, pro_win, recommendations};
use std::collections::HashMap;
use std::sync::Arc;

fn heroes() -> HeroCatalog {
    HeroCatalog::new(vec![HeroInfo {
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![],
        abilities: vec![AbilityInfo {
            name: "zuus_arc_lightning".into(),
            display_name: "Arc Lightning".into(),
        }],
    }])
}

fn abilities() -> AbilityIdCatalog {
    AbilityIdCatalog::new(HashMap::from([
        (5110, "zuus_arc_lightning".to_owned()),
        (6000, "special_bonus_spell_amplify_8".to_owned()),
        (7000, "zuus_new_ability".to_owned()),
    ]))
}

fn ready(mode: GameMode, wins: Vec<ProWin>) -> RecommendationStatus {
    let pro = pro_builds(wins, 12, Some(DataOrigin::Live));
    RecommendationStatus::Ready(Arc::new(recommendations(mode, pro, vec![])))
}

fn content(status: &RecommendationStatus) -> Vec<Vec<RowContent>> {
    abilities_content(status, &heroes(), &abilities(), &Redactor::default())
}

#[test]
fn each_win_shows_its_real_order_level_by_level() {
    let wins = vec![
        pro_win(2, PlayerBuild::default(), Some(vec![5110, 6000, 1])),
        pro_win(1, PlayerBuild::default(), None),
    ];

    let sections = content(&ready(GameMode::AllPick, wins));

    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0][0].title, "Zeus · Escolha Livre · patch 7.41");
    assert_eq!(
        sections[2][0],
        RowContent::new(
            Level::Info,
            "Vitória profissional: Liga Teste",
            "duração 40:00 · 1 Arc Lightning → 2 Talento → 3 habilidade 1"
        )
    );
    assert_eq!(
        sections[2][1].subtitle,
        "duração 40:00 · Sem ordem de habilidades: a partida não foi analisada pelo OpenDota."
    );
    assert_eq!(sections[2][2].title, "2 vitórias recentes encontradas");
}

#[test]
fn without_wins_the_tab_says_none_was_found() {
    let sections = content(&ready(GameMode::AllPick, vec![]));

    assert_eq!(
        sections[2][0],
        RowContent::new(
            Level::Info,
            "Nenhuma vitória recente encontrada",
            "Entre as 12 partidas de liga recentes examinadas; derrotas, outros patches e partidas sem build ficam de fora."
        )
    );
    assert!(sections[2][1].title.starts_with("Fonte: OpenDota"));
}

#[test]
fn turbo_and_bots_add_their_notes() {
    let turbo = content(&ready(GameMode::Turbo, vec![]));
    let bots = content(&ready(GameMode::Bots, vec![]));

    assert_eq!(turbo[1][1].title, "Partidas profissionais não são turbo");
    assert_eq!(
        bots[1][1].title,
        "Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores."
    );
    assert_eq!(turbo[1][0].title, "A ordem não considera posição.");
}

#[test]
fn ability_names_fall_back_to_internal_name_and_id() {
    assert_eq!(ability_name(5110, &heroes(), &abilities()), "Arc Lightning");
    assert_eq!(
        ability_name(7000, &heroes(), &abilities()),
        "zuus_new_ability"
    );
    assert_eq!(ability_name(1, &heroes(), &abilities()), "habilidade 1");
}

#[test]
fn not_ready_shows_only_the_status() {
    let sections = content(&RecommendationStatus::Idle);

    assert_eq!(sections[0][0].title, "Nenhum herói para recomendar");
    assert!(sections[1].is_empty() && sections[2].is_empty());
}
