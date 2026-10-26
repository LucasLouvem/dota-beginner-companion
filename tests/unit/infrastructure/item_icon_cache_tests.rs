//! Testes unitários de `src/infrastructure/item_icon_cache.rs`, em pastas
//! temporárias.

use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn saved_image_is_loaded_back_and_missing_is_none() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cache = ItemIconCache::new(temp_dir.path().join("item-icons"));

    assert_eq!(cache.load("item_blink").unwrap(), None);
    cache.save("item_blink", b"png").unwrap();

    assert_eq!(cache.load("item_blink").unwrap(), Some(b"png".to_vec()));
    assert!(temp_dir.path().join("item-icons/item_blink.png").exists());
    assert!(
        !temp_dir
            .path()
            .join("item-icons/item_blink.png.tmp")
            .exists()
    );
}

#[test]
fn unexpected_item_names_never_become_paths() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cache = ItemIconCache::new(temp_dir.path().to_path_buf());

    for item in ["../item_blink", "item_", "blink", "item_Blink", "item_a/b"] {
        assert!(
            matches!(cache.load(item), Err(ItemIconError::UnexpectedPath { .. })),
            "{item}"
        );
        assert!(cache.save(item, b"png").is_err(), "{item}");
    }
}

#[test]
fn save_in_a_read_only_folder_is_a_cache_error_without_partial_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let locked = temp_dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::write(locked.join("probe"), "").is_ok() {
        // Executando como root: permissões não se aplicam.
        return;
    }
    let cache = ItemIconCache::new(locked.clone());

    let result = cache.save("item_blink", b"png");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();

    assert!(
        matches!(result, Err(ItemIconError::Cache { .. })),
        "{result:?}"
    );
    assert_eq!(fs::read_dir(&locked).unwrap().count(), 0);
}
