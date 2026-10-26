//! Vitórias públicas em memória.

use std::sync::Mutex;

use std::cmp::Reverse;

use crate::application::ports::{PublicBuildStore, PublicWin, StorageError};

/// Guarda as vitórias e o cursor como o SQLite, sem banco: descarta a mesma
/// partida e herói repetidos e lê por herói, modo e patch, das mais recentes.
#[derive(Default)]
pub struct MemoryPublicBuildStore {
    wins: Mutex<Vec<PublicWin>>,
    cursor: Mutex<Option<u64>>,
}

impl MemoryPublicBuildStore {
    pub fn starting_at(cursor: u64) -> Self {
        let store = Self::default();
        *store.cursor.lock().unwrap() = Some(cursor);
        store
    }

    pub fn saved(&self) -> Vec<PublicWin> {
        self.wins.lock().unwrap().clone()
    }

    pub fn current_cursor(&self) -> Option<u64> {
        *self.cursor.lock().unwrap()
    }
}

impl PublicBuildStore for MemoryPublicBuildStore {
    fn save_match(&self, wins: &[PublicWin], cursor: u64) -> Result<(), StorageError> {
        let mut saved = self.wins.lock().unwrap();
        for win in wins {
            let repeated = saved.iter().any(|existing| {
                existing.match_id == win.match_id && existing.hero_id == win.hero_id
            });
            if !repeated {
                saved.push(win.clone());
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

    fn recent_wins(
        &self,
        hero_id: u32,
        game_modes: &[u32],
        patch_id: u32,
        limit: usize,
    ) -> Result<Vec<PublicWin>, StorageError> {
        let mut wins: Vec<PublicWin> = self
            .wins
            .lock()
            .unwrap()
            .iter()
            .filter(|win| {
                win.hero_id == hero_id
                    && win.patch_id == patch_id
                    && game_modes.contains(&win.game_mode)
            })
            .cloned()
            .collect();
        wins.sort_by_key(|win| {
            (
                win.start_time.is_none(),
                Reverse(win.start_time),
                Reverse(win.match_id),
            )
        });
        wins.truncate(limit);
        Ok(wins)
    }

    fn purge_other_patches(&self, patch_id: u32) -> Result<usize, StorageError> {
        let mut saved = self.wins.lock().unwrap();
        let before = saved.len();
        saved.retain(|win| win.patch_id == patch_id);
        Ok(before - saved.len())
    }
}
