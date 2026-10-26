//! Testes unitários de `src/domain/item_catalog.rs`.

use super::*;

fn catalog() -> ItemCatalog {
    ItemCatalog::new(vec![
        ItemInfo {
            id: 0,
            name: "item_satanic".into(),
            display_name: "Satanic".into(),
            cost: None,
            image_path: None,
            components: vec![
                "item_reaver".into(),
                "item_claymore".into(),
                "item_lifesteal".into(),
            ],
        },
        ItemInfo {
            id: 0,
            name: "item_tango".into(),
            display_name: "Tango".into(),
            cost: None,
            image_path: None,
            components: vec![],
        },
    ])
}

#[test]
fn finds_known_item_by_internal_name() {
    assert_eq!(
        catalog()
            .by_name("item_satanic")
            .map(|item| item.display_name.as_str()),
        Some("Satanic")
    );
}

#[test]
fn unknown_item_is_not_found() {
    assert_eq!(catalog().by_name("item_from_a_future_patch"), None);
}

#[test]
fn display_name_falls_back_to_internal_name_for_unknown_item() {
    let catalog = catalog();
    assert_eq!(catalog.display_name("item_tango"), "Tango");
    assert_eq!(
        catalog.display_name("item_from_a_future_patch"),
        "item_from_a_future_patch"
    );
}

#[test]
fn components_of_recipe_item_are_listed_in_source_order() {
    assert_eq!(
        catalog().components("item_satanic"),
        ["item_reaver", "item_claymore", "item_lifesteal"]
    );
}

#[test]
fn item_without_recipe_or_unknown_item_has_no_components() {
    let catalog = catalog();
    assert!(catalog.components("item_tango").is_empty());
    assert!(catalog.components("item_from_a_future_patch").is_empty());
}

fn priced(name: &str, cost: Option<u32>) -> ItemInfo {
    ItemInfo {
        id: 0,
        name: name.into(),
        display_name: name.into(),
        cost,
        image_path: None,
        components: vec![],
    }
}

#[test]
fn cost_comes_from_the_source_when_it_has_one() {
    let catalog = ItemCatalog::new(vec![
        priced("item_blink", Some(2250)),
        priced("item_no_cost", None),
    ]);
    assert_eq!(catalog.cost("item_blink"), Some(2250));
    assert_eq!(catalog.cost("item_no_cost"), None);
    assert_eq!(catalog.cost("item_unknown"), None);
}
