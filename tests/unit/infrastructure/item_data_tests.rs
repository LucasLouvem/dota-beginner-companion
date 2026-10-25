//! Testes unitários de `src/infrastructure/item_data.rs`.

use super::*;

#[test]
fn embedded_catalog_has_every_item_of_the_snapshot() {
    let catalog = embedded_catalog().expect("o recorte embutido é válido");
    assert_eq!(catalog.len(), 491);
}

#[test]
fn embedded_catalog_has_satanic_recipe_as_in_source() {
    let catalog = embedded_catalog().unwrap();
    assert_eq!(catalog.display_name("item_satanic"), "Satanic");
    assert_eq!(
        catalog.components("item_satanic"),
        ["item_reaver", "item_claymore", "item_lifesteal"]
    );
}

#[test]
fn embedded_catalog_has_items_seen_in_the_dragon_knight_fixture() {
    let catalog = embedded_catalog().unwrap();
    for (item, display_name) in [
        ("item_tango", "Tango"),
        ("item_quelling_blade", "Quelling Blade"),
        ("item_soul_ring", "Soul Ring"),
        ("item_tpscroll", "Town Portal Scroll"),
    ] {
        assert_eq!(catalog.display_name(item), display_name, "{item}");
    }
}

#[test]
fn embedded_components_are_never_empty_names() {
    let catalog = embedded_catalog().unwrap();
    for item in ["item_pipe", "item_sphere", "item_octarine_core"] {
        assert!(
            catalog
                .components(item)
                .iter()
                .all(|component| component.starts_with("item_") && component.len() > 5),
            "{item}"
        );
    }
}

#[test]
fn fails_with_typed_error_when_data_is_not_an_item_list() {
    let data_error = parse_catalog(r#"{"items": []}"#).unwrap_err();
    assert_eq!(data_error.0.classify(), serde_json::error::Category::Data);
    assert_eq!(
        data_error.to_diagnostic(&Redactor::default()).title,
        UiText::new("Dados dos itens indisponíveis")
    );
}

#[test]
fn fails_with_syntax_error_when_data_is_not_json() {
    let data_error = parse_catalog("not json").unwrap_err();
    assert_eq!(data_error.0.classify(), serde_json::error::Category::Syntax);
}
