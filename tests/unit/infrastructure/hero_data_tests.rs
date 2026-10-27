//! Testes unitários de `src/infrastructure/hero_data.rs`.

use super::*;

#[test]
fn embedded_catalog_has_every_hero_of_the_snapshot() {
    let catalog = embedded_catalog().expect("o recorte embutido é válido");
    assert_eq!(catalog.len(), 127);
}

#[test]
fn embedded_catalog_has_zeus_as_in_source() {
    let catalog = embedded_catalog().unwrap();
    let zeus = catalog
        .by_name("npc_dota_hero_zuus")
        .expect("Zeus está no recorte");
    assert_eq!(zeus.id, 22);
    assert_eq!(zeus.display_name, "Zeus");
    assert_eq!(zeus.primary_attribute, PrimaryAttribute::Intelligence);
    assert_eq!(zeus.attack_type, AttackType::Ranged);
    assert_eq!(zeus.roles, vec![HeroRole::Nuker, HeroRole::Carry]);
    assert_eq!(
        zeus.ability_display_name("zuus_thundergods_wrath"),
        Some("Thundergod's Wrath")
    );
}

#[test]
fn embedded_catalog_has_every_ability_seen_in_the_dragon_knight_fixture() {
    let catalog = embedded_catalog().unwrap();
    let dragon_knight = catalog.by_id(49).expect("Dragon Knight está no recorte");
    assert_eq!(dragon_knight.name, "npc_dota_hero_dragon_knight");
    assert_eq!(dragon_knight.primary_attribute, PrimaryAttribute::Strength);
    assert_eq!(dragon_knight.attack_type, AttackType::Melee);
    for (ability, display_name) in [
        ("dragon_knight_breathe_fire", "Breathe Fire"),
        ("dragon_knight_dragon_tail", "Dragon Tail"),
        ("dragon_knight_wyrms_wrath", "Wyrm's Wrath"),
        ("dragon_knight_dragon_blood", "Dragon Blood"),
        ("dragon_knight_elder_dragon_form", "Elder Dragon Form"),
    ] {
        assert_eq!(
            dragon_knight.ability_display_name(ability),
            Some(display_name),
            "{ability}"
        );
    }
}

#[test]
fn fails_with_typed_error_when_data_is_not_a_hero_list() {
    let data_error = parse_catalog(r#"{"heroes": []}"#).unwrap_err();
    assert_eq!(data_error.0.classify(), serde_json::error::Category::Data);
    assert_eq!(
        data_error.to_diagnostic(&Redactor::default()).title,
        UiText::new("Dados dos heróis indisponíveis")
    );
}

#[test]
fn embedded_zeus_and_his_ability_have_image_paths() {
    let catalog = embedded_catalog().unwrap();
    let zeus = catalog.by_name("npc_dota_hero_zuus").unwrap();
    assert_eq!(
        zeus.image_path.as_deref(),
        Some("/apps/dota2/images/dota_react/heroes/zuus.png")
    );
    assert_eq!(
        catalog.image_path_of("zuus_arc_lightning"),
        Some("/apps/dota2/images/dota_react/abilities/zuus_arc_lightning.png")
    );
    assert_eq!(
        catalog.image_path_of("npc_dota_hero_zuus"),
        zeus.image_path.as_deref()
    );
    assert_eq!(catalog.image_path_of("not_an_ability"), None);
}
