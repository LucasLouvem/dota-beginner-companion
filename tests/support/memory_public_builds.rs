//! Builds públicas em memória.

use std::sync::Mutex;

use crate::application::ports::{PublicBuild, PublicBuildStore, StorageError};
use crate::domain::builds::MatchBuild;

/// Guarda as builds e o cursor como o SQLite, sem banco: ignora a mesma
/// partida e herói repetidos e filtra a leitura por herói, modo e patch.
#[derive(Default)]
pub struct MemoryPublicBuildStore {
    builds: Mutex<Vec<PublicBuild>>,
    cursor: Mutex<Option<u64>>,
}

impl MemoryPublicBuildStore {
    pub fn starting_at(cursor: u64) -> Self {
        let store = Self::default();
        *store.cursor.lock().unwrap() = Some(cursor);
        store
    }

    pub fn saved(&self) -> Vec<PublicBuild> {
        self.builds.lock().unwrap().clone()
    }

    pub fn current_cursor(&self) -> Option<u64> {
        *self.cursor.lock().unwrap()
    }
}

impl PublicBuildStore for MemoryPublicBuildStore {
    fn save_match(&self, builds: &[PublicBuild], cursor: u64) -> Result<(), StorageError> {
        let mut saved = self.builds.lock().unwrap();
        for build in builds {
            let repeated = saved.iter().any(|existing| {
                existing.match_id == build.match_id && existing.hero_id == build.hero_id
            });
            if !repeated {
                saved.push(build.clone());
            }
        }
        *self.cursor.lock().unwrap() = Some(cursor);
        Ok(())
    }

    fn cursor(&self) -> Result<Option<u64>, StorageError> {
        Ok(self.current_cursor())
    }

    fn reset_cursor(&self) -> Result<(), StorageError> {
        *self.cursor.lock().unwrap() = None;
        Ok(())
    }

    fn builds(
        &self,
        hero_id: u32,
        game_modes: &[u32],
        patch_id: u32,
    ) -> Result<Vec<(MatchBuild, bool)>, StorageError> {
        Ok(self
            .builds
            .lock()
            .unwrap()
            .iter()
            .filter(|build| {
                build.hero_id == hero_id
                    && build.patch_id == patch_id
                    && game_modes.contains(&build.game_mode)
            })
            .map(|build| (build.build.clone(), build.won))
            .collect())
    }

    fn purge_other_patches(&self, patch_id: u32) -> Result<usize, StorageError> {
        let mut saved = self.builds.lock().unwrap();
        let before = saved.len();
        saved.retain(|build| build.patch_id == patch_id);
        Ok(before - saved.len())
    }
}
