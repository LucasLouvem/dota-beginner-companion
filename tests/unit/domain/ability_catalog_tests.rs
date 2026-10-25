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
