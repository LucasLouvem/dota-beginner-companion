//! Testes unitários de `src/domain/ability_catalog.rs`.

use super::*;

fn catalog() -> AbilityIdCatalog {
    AbilityIdCatalog::new(HashMap::from([
        (5110, "zuus_arc_lightning".to_owned()),
        (5111, "zuus_lightning_bolt".to_owned()),
    ]))
}

#[test]
fn finds_ability_name_by_id() {
    assert_eq!(catalog().name(5110), Some("zuus_arc_lightning"));
}

#[test]
fn unknown_ability_id_has_no_name() {
    assert_eq!(catalog().name(1), None);
    assert!(AbilityIdCatalog::default().is_empty());
}

#[test]
fn talents_are_special_bonus_abilities_and_unknown_ids_are_not() {
    let catalog = AbilityIdCatalog::new(HashMap::from([
        (5110, "zuus_arc_lightning".to_owned()),
        (6000, "special_bonus_spell_amplify_8".to_owned()),
    ]));
    assert!(catalog.is_talent(6000));
    assert!(!catalog.is_talent(5110));
    assert!(!catalog.is_talent(1));
}
