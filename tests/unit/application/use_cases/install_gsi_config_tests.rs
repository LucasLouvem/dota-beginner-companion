//! Testes unitários de `src/application/use_cases/install_gsi_config.rs`.

use super::*;
use crate::test_support::MemoryFiles;
use std::path::PathBuf;

fn endpoint() -> GsiEndpoint {
    GsiEndpoint {
        port: 53000,
        token: "secret".into(),
    }
}

fn expected_config_path(library_root: &Path) -> PathBuf {
    DotaInstall::game_root_for(library_root)
        .join("game/dota/cfg/gamestate_integration")
        .join(gsi_config::CONFIG_FILE_NAME)
}

const OTHER_APP_CONFIG: &str = "\"old\" { \"uri\" \"http://elsewhere\" }";

#[test]
fn creates_config_when_none_exists() {
    let files = MemoryFiles::default();
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);

    assert_eq!(
        install_gsi_config(&files, library_root, &endpoint(), false),
        InstallOutcome::Created(config_path.clone())
    );
    assert_eq!(
        files.get(&config_path),
        Some(gsi_config::render(&endpoint()))
    );
}

#[test]
fn reports_unchanged_when_config_already_matches() {
    let files = MemoryFiles::default();
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);
    files.put(&config_path, &gsi_config::render(&endpoint()));

    assert_eq!(
        install_gsi_config(&files, library_root, &endpoint(), false),
        InstallOutcome::Unchanged(config_path)
    );
}

#[test]
fn asks_confirmation_and_keeps_file_when_existing_config_differs() {
    let files = MemoryFiles::default();
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);
    files.put(&config_path, OTHER_APP_CONFIG);

    assert_eq!(
        install_gsi_config(&files, library_root, &endpoint(), false),
        InstallOutcome::NeedsConfirmation(config_path.clone())
    );
    assert_eq!(files.get(&config_path).as_deref(), Some(OTHER_APP_CONFIG));
}

#[test]
fn backs_up_and_replaces_when_overwrite_is_confirmed() {
    let files = MemoryFiles::default();
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);
    files.put(&config_path, OTHER_APP_CONFIG);

    let outcome = install_gsi_config(&files, library_root, &endpoint(), true);
    let InstallOutcome::Replaced { backup, .. } = outcome else {
        panic!("esperava Replaced, veio {outcome:?}");
    };
    assert_eq!(files.get(&backup).as_deref(), Some(OTHER_APP_CONFIG));
    assert_eq!(
        files.get(&config_path),
        Some(gsi_config::render(&endpoint()))
    );
}
