//! Testes unitários de `src/infrastructure/ability_id_data.rs`.

use super::*;

#[test]
fn embedded_catalog_has_every_id_of_the_snapshot() {
    let catalog = embedded_catalog().expect("o recorte embutido é válido");
    // 3157 chaves, uma delas com dois IDs.
    assert_eq!(catalog.len(), 3158);
    assert_eq!(catalog.name(0), Some("dota_base_ability"));
}

#[test]
fn reads_ids_as_numbers() {
    let catalog = parse_catalog(r#"{"5110": "zuus_arc_lightning"}"#).unwrap();
    assert_eq!(catalog.name(5110), Some("zuus_arc_lightning"));
}

#[test]
fn fails_with_typed_error_when_an_id_is_not_a_number() {
    let error = parse_catalog(r#"{"abc": "zuus_arc_lightning"}"#).unwrap_err();
    assert!(
        matches!(&error, AbilityIdDataError::InvalidId(id) if id == "abc"),
        "{error:?}"
    );
}

#[test]
fn fails_with_typed_error_when_data_is_not_json() {
    let error = parse_catalog("[").unwrap_err();
    assert!(matches!(error, AbilityIdDataError::Json(_)), "{error:?}");
    let diagnostic = error.to_diagnostic(&Redactor::default());
    assert_eq!(
        diagnostic.title,
        UiText::new("Dados das habilidades indisponíveis")
    );
}

#[test]
fn key_with_ids_separated_by_comma_gives_the_same_name_to_each_id() {
    let catalog = embedded_catalog().unwrap();
    assert_eq!(catalog.name(3060), Some("templar_assassin_hidden_gates"));
    assert_eq!(catalog.name(1617), Some("templar_assassin_hidden_gates"));
}

#[test]
fn fails_with_typed_error_when_one_of_the_ids_in_a_key_is_not_a_number() {
    let error = parse_catalog(r#"{"3060,x": "templar_assassin_hidden_gates"}"#).unwrap_err();
    assert!(
        matches!(&error, AbilityIdDataError::InvalidId(id) if id == "3060,x"),
        "{error:?}"
    );
}
