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
        for root in STEAM_ROOTS.iter().map(|r| self.home.join(r)) {
            if !root.join("steamapps").is_dir() {
                continue;
            }
            candidates.push(root.clone());
            for vdf in [
                root.join("steamapps/libraryfolders.vdf"),
                root.join("config/libraryfolders.vdf"),
            ] {
                candidates.extend(library_paths_from_vdf(&vdf));
            }
        }
        candidates.extend(self.extra_libraries.iter().cloned());

        let mut seen = HashSet::new();
        candidates
            .into_iter()
            .filter(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
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

    #[test]
    fn finds_dota_in_secondary_library() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let steam = home.join(".local/share/Steam");
        let games = tmp.path().join("mnt/jogos/SteamLibrary");

        create_dir_all(steam.join("steamapps")).unwrap();
        let vdf = include_str!("../../tests/fixtures/libraryfolders.vdf")
            .replace("/home/user/.local/share/Steam", steam.to_str().unwrap())
            .replace("/mnt/jogos/SteamLibrary", games.to_str().unwrap());
        fs::write(steam.join("steamapps/libraryfolders.vdf"), vdf).unwrap();

        let dota = DotaInstall::game_root_for(&games);
        create_dir_all(dota.join("game/dota/cfg")).unwrap();
        create_dir_all(dota.join("game/bin/linuxsteamrt64")).unwrap();

        // Biblioteca extra repetida não deve gerar duplicata.
        let locator = SteamLocator::new(home, vec![games.clone()]);
        let result = locator.locate();

        assert_eq!(result.searched_libraries, vec![steam, games.clone()]);
        assert_eq!(result.installs.len(), 1);
        let install = &result.installs[0];
        assert_eq!(install.library, games);
        assert!(install.native_binary);
        assert!(!install.proton_prefix);
        assert!(
            install
                .gsi_dir()
                .ends_with("game/dota/cfg/gamestate_integration")
        );
    }

    #[test]
    fn empty_home_finds_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let result = SteamLocator::new(tmp.path().to_path_buf(), vec![]).locate();
        assert!(result.searched_libraries.is_empty());
        assert!(result.installs.is_empty());
    }
}
