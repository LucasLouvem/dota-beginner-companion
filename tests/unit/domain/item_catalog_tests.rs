//! Testes unitários de `src/domain/item_catalog.rs`.

use super::*;

fn catalog() -> ItemCatalog {
    ItemCatalog::new(vec![
        ItemInfo {
            name: "item_satanic".into(),
            display_name: "Satanic".into(),
            cost: None,
            components: vec![
                "item_reaver".into(),
                "item_claymore".into(),
                "item_lifesteal".into(),
            ],
        },
        ItemInfo {
            name: "item_tango".into(),
            display_name: "Tango".into(),
            cost: None,
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

fn priced(name: &str, cost: Option<u32>, components: &[&str]) -> ItemInfo {
    ItemInfo {
        name: name.into(),
        display_name: name.into(),
        cost,
        components: components
            .iter()
            .map(|component| (*component).into())
            .collect(),
    }
}

fn priced_catalog() -> ItemCatalog {
    ItemCatalog::new(vec![
        priced("item_exact", Some(2000), &["item_a", "item_b"]),
        priced("item_below", Some(1999), &["item_a", "item_b"]),
        priced("item_no_recipe", Some(4000), &[]),
        priced("item_no_cost", None, &["item_a"]),
    ])
}

#[test]
fn item_with_recipe_and_cost_of_at_least_2000_is_major() {
    assert!(priced_catalog().is_major_item("item_exact"));
    assert_eq!(priced_catalog().cost("item_exact"), Some(2000));
}

#[test]
fn cheaper_item_item_without_recipe_or_cost_and_unknown_item_are_not_major() {
    let catalog = priced_catalog();
    assert!(!catalog.is_major_item("item_below"));
    assert!(!catalog.is_major_item("item_no_recipe"));
    assert!(!catalog.is_major_item("item_no_cost"));
    assert!(!catalog.is_major_item("item_unknown"));
    assert_eq!(catalog.cost("item_unknown"), None);
}
