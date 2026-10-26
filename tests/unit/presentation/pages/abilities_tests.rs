//! Testes unitários de `src/presentation/pages/abilities.rs`.

use super::*;
use crate::application::use_cases::fallback::DataOrigin;
use crate::domain::builds::AbilityOrder;
use crate::domain::hero_catalog::{AbilityInfo, AttackType, HeroInfo, PrimaryAttribute};
use crate::test_support::{pro_builds, recommendations};
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

fn ready(mode: GameMode, order: Option<AbilityOrder>) -> RecommendationStatus {
    let mut pro = pro_builds(vec![], 30, Some(DataOrigin::Live));
    pro.ability_order = order;
    RecommendationStatus::Ready(Arc::new(recommendations(mode, pro, vec![])))
}

fn content(status: &RecommendationStatus) -> Vec<Vec<RowContent>> {
    abilities_content(status, &heroes(), &abilities(), &Redactor::default())
}

#[test]
fn most_common_order_lists_each_level_with_the_sample() {
    let order = AbilityOrder {
        ability_ids: vec![5110, 6000],
        matches: 12,
        analyzed: 30,
    };

    let sections = content(&ready(GameMode::AllPick, Some(order)));

    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0][0].title, "Zeus · Escolha Livre · patch 7.41");
    assert_eq!(sections[2][0].title, "Nível 1: Arc Lightning");
    assert_eq!(sections[2][1].title, "Nível 2: Talento");
    assert_eq!(
        sections[2][0].subtitle,
        "Mais comum: 12 de 30 partidas profissionais válidas"
    );
}

#[test]
fn without_a_common_order_the_sample_is_insufficient() {
    let sections = content(&ready(GameMode::AllPick, None));

    assert_eq!(sections[2][0].title, "Amostra insuficiente");
    assert_eq!(
        sections[2][0].subtitle,
        "Nenhuma ordem dos 10 primeiros níveis aparece em pelo menos 10 partidas entre 30 partidas profissionais válidas."
    );
}

#[test]
fn turbo_and_bots_add_their_notes() {
    let turbo = content(&ready(GameMode::Turbo, None));
    let bots = content(&ready(GameMode::Bots, None));

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
