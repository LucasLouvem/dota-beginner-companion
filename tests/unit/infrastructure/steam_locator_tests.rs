//! Testes unitários de `src/infrastructure/steam_locator.rs`.

use super::*;
use std::fs::create_dir_all;

/// Home falsa com a Steam padrão e uma biblioteca secundária contendo o Dota.
struct FakeSteam {
    _temp_dir: tempfile::TempDir,
    home: PathBuf,
    steam_root: PathBuf,
    games_library: PathBuf,
}

fn fake_steam(libraryfolders: &str) -> FakeSteam {
    let temp_dir = tempfile::tempdir().unwrap();
    let home = temp_dir.path().join("home");
    let steam_root = home.join(".local/share/Steam");
    let games_library = temp_dir.path().join("mnt/jogos/SteamLibrary");

    create_dir_all(steam_root.join("steamapps")).unwrap();
    let vdf_contents = libraryfolders
        .replace(
            "/home/user/.local/share/Steam",
            steam_root.to_str().unwrap(),
        )
        .replace("/mnt/jogos/SteamLibrary", games_library.to_str().unwrap());
    fs::write(
        steam_root.join("steamapps/libraryfolders.vdf"),
        vdf_contents,
    )
    .unwrap();

    let dota_root = DotaInstall::game_root_for(&games_library);
    create_dir_all(dota_root.join("game/dota/cfg")).unwrap();
    create_dir_all(dota_root.join("game/bin/linuxsteamrt64")).unwrap();

    FakeSteam {
        _temp_dir: temp_dir,
        home,
        steam_root,
        games_library,
    }
}

const LIBRARYFOLDERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/libraryfolders.vdf"
));

#[test]
fn finds_native_dota_in_library_listed_in_libraryfolders_vdf() {
    let steam = fake_steam(LIBRARYFOLDERS);
    let result = SteamLocator::new(steam.home.clone(), vec![]).locate();

    assert_eq!(
        result.searched_libraries,
        vec![steam.steam_root.clone(), steam.games_library.clone()]
    );
    assert_eq!(result.installs.len(), 1);
    let install = &result.installs[0];
    assert_eq!(install.library, steam.games_library);
    assert!(install.native_binary);
    assert!(!install.proton_prefix);
}

#[test]
fn does_not_duplicate_extra_library_already_in_libraryfolders_vdf() {
    let steam = fake_steam(LIBRARYFOLDERS);
    let result = SteamLocator::new(steam.home.clone(), vec![steam.games_library.clone()]).locate();

    assert_eq!(result.searched_libraries.len(), 2);
    assert_eq!(result.installs.len(), 1);
}

#[test]
fn ignores_malformed_libraryfolders_vdf_and_still_uses_extra_libraries() {
    let steam = fake_steam("\"libraryfolders\" {");
    let result = SteamLocator::new(steam.home.clone(), vec![steam.games_library.clone()]).locate();

    assert_eq!(
        result.searched_libraries,
        vec![steam.steam_root.clone(), steam.games_library.clone()]
    );
    assert_eq!(result.installs.len(), 1);
}

#[test]
fn finds_nothing_when_home_has_no_steam() {
    let temp_dir = tempfile::tempdir().unwrap();
    let result = SteamLocator::new(temp_dir.path().to_path_buf(), vec![]).locate();
    assert!(result.searched_libraries.is_empty());
    assert!(result.installs.is_empty());
}
