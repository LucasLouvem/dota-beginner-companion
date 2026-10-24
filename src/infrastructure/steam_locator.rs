//! Localiza bibliotecas Steam e instalações do Dota 2 lendo
//! `libraryfolders.vdf`. Só leitura de arquivos de configuração da Steam.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::application::ports::{DotaLocator, LocatorResult};
use crate::domain::dota_install::{DOTA_APP_ID, DotaInstall};
use crate::domain::keyvalues;

/// Raízes comuns da Steam, relativas à home do usuário.
const STEAM_ROOTS: &[&str] = &[
    ".local/share/Steam",
    ".steam/steam",
    ".steam/root",
    // Steam via Flatpak
    ".var/app/com.valvesoftware.Steam/.local/share/Steam",
    ".var/app/com.valvesoftware.Steam/data/Steam",
];

/// Localizador de bibliotecas Steam e instalações do Dota.
pub struct SteamLocator {
    home: PathBuf,
    /// Bibliotecas extras informadas pelo usuário (ex.: `/mnt/jogos/SteamLibrary`).
    extra_libraries: Vec<PathBuf>,
}

impl SteamLocator {
    /// Procura a partir da home informada, somando as bibliotecas extras.
    pub fn new(home: PathBuf, extra_libraries: Vec<PathBuf>) -> Self {
        Self {
            home,
            extra_libraries,
        }
    }

    fn libraries(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        for root in STEAM_ROOTS.iter().map(|relative| self.home.join(relative)) {
            if !root.join("steamapps").is_dir() {
                continue;
            }
            candidates.push(root.clone());
            for vdf_path in [
                root.join("steamapps/libraryfolders.vdf"),
                root.join("config/libraryfolders.vdf"),
            ] {
                candidates.extend(library_paths_from_vdf(&vdf_path));
            }
        }
        candidates.extend(self.extra_libraries.iter().cloned());

        let mut seen = HashSet::new();
        candidates
            .into_iter()
            .filter(|library| {
                seen.insert(fs::canonicalize(library).unwrap_or_else(|_| library.clone()))
            })
            .collect()
    }
}

impl DotaLocator for SteamLocator {
    fn locate(&self) -> LocatorResult {
        let searched_libraries = self.libraries();
        let installs = searched_libraries
            .iter()
            .filter_map(|library| inspect_library(library))
            .collect();
        LocatorResult {
            searched_libraries,
            installs,
        }
    }
}

fn library_paths_from_vdf(path: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(root) = keyvalues::parse(&text) else {
        return Vec::new();
    };
    let Some(folders) = root.get("libraryfolders") else {
        return Vec::new();
    };
    folders
        .entries()
        .iter()
        .filter_map(|(_, folder)| folder.get("path")?.as_text().map(PathBuf::from))
        .collect()
}

fn inspect_library(library: &Path) -> Option<DotaInstall> {
    let game_root = DotaInstall::game_root_for(library);
    if !game_root.join("game/dota").is_dir() {
        return None;
    }
    Some(DotaInstall {
        library: library.to_path_buf(),
        native_binary: game_root.join("game/bin/linuxsteamrt64").is_dir(),
        proton_prefix: library
            .join("steamapps/compatdata")
            .join(DOTA_APP_ID.to_string())
            .is_dir(),
        game_root,
    })
}

#[cfg(test)]
mod tests {
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

    const LIBRARYFOLDERS: &str = include_str!("../../tests/fixtures/libraryfolders.vdf");

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
        let result =
            SteamLocator::new(steam.home.clone(), vec![steam.games_library.clone()]).locate();

        assert_eq!(result.searched_libraries.len(), 2);
        assert_eq!(result.installs.len(), 1);
    }

    #[test]
    fn ignores_malformed_libraryfolders_vdf_and_still_uses_extra_libraries() {
        let steam = fake_steam("\"libraryfolders\" {");
        let result =
            SteamLocator::new(steam.home.clone(), vec![steam.games_library.clone()]).locate();

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
}
