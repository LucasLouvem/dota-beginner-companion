//! Testes unitários de `src/domain/hero_catalog.rs`.

use super::*;

fn zeus() -> HeroInfo {
    HeroInfo {
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![HeroRole::Nuker, HeroRole::Carry],
        abilities: vec![AbilityInfo {
            name: "zuus_arc_lightning".into(),
            display_name: "Arc Lightning".into(),
        }],
    }
}

#[test]
fn finds_hero_by_internal_name_and_by_id() {
    let catalog = HeroCatalog::new(vec![zeus()]);
    assert_eq!(catalog.by_name("npc_dota_hero_zuus"), Some(&zeus()));
    assert_eq!(catalog.by_id(22), Some(&zeus()));
}

#[test]
fn unknown_hero_is_not_found() {
    let catalog = HeroCatalog::new(vec![zeus()]);
    assert_eq!(catalog.by_name("npc_dota_hero_desconhecido"), None);
    assert_eq!(catalog.by_id(0), None);
}

#[test]
fn ability_display_name_comes_from_hero_abilities() {
    assert_eq!(
        zeus().ability_display_name("zuus_arc_lightning"),
        Some("Arc Lightning")
    );
    assert_eq!(zeus().ability_display_name("zuus_static_field"), None);
}

#[test]
fn catalog_finds_ability_display_name_of_any_hero() {
    let catalog = HeroCatalog::new(vec![zeus()]);
    assert_eq!(
        catalog.ability_display_name("zuus_arc_lightning"),
        Some("Arc Lightning")
    );
    assert_eq!(catalog.ability_display_name("unknown_ability"), None);
}

#[test]
fn source_codes_map_to_known_values_and_keep_unknown_ones() {
    assert_eq!(
        PrimaryAttribute::from_code("all"),
        PrimaryAttribute::Universal
    );
    assert_eq!(
        PrimaryAttribute::from_code("luck"),
        PrimaryAttribute::Other("luck".into())
    );
    assert_eq!(AttackType::from_name("Melee"), AttackType::Melee);
    assert_eq!(HeroRole::from_name("Support"), HeroRole::Support);
    assert_eq!(
        HeroRole::from_name("Jungler"),
        HeroRole::Other("Jungler".into())
    );
}
