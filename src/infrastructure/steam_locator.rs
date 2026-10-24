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
#[path = "../../tests/unit/infrastructure/steam_locator_tests.rs"]
mod tests;
