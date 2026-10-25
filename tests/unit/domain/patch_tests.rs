//! Testes unitários de `src/domain/patch.rs`.

use super::*;

fn patch(id: u32, name: &str) -> Patch {
    Patch {
        id,
        name: name.into(),
    }
}

#[test]
fn current_patch_is_the_one_with_the_highest_id_in_any_order() {
    let catalog = PatchCatalog::new(vec![
        patch(60, "7.41"),
        patch(58, "7.39"),
        patch(59, "7.40"),
    ]);
    assert_eq!(catalog.current(), Some(&patch(60, "7.41")));
}

#[test]
fn finds_patch_by_id_and_not_an_unknown_one() {
    let catalog = PatchCatalog::new(vec![patch(59, "7.40"), patch(60, "7.41")]);
    assert_eq!(catalog.by_id(59), Some(&patch(59, "7.40")));
    assert_eq!(catalog.by_id(61), None);
}

#[test]
fn empty_catalog_has_no_current_patch() {
    assert_eq!(PatchCatalog::default().current(), None);
}
