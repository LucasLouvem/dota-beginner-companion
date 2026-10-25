//! Cache em memória, com o mesmo comportamento de validade do SQLite.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::application::ports::{CacheStore, CachedValue, Freshness, StorageError};

/// Cache sem banco: guarda valor e horário de gravação, e `get` marca como
/// antigo pelo mesmo critério do SQLite (`Freshness::of`).
#[derive(Default)]
pub struct MemoryCache {
    entries: Mutex<HashMap<String, (String, SystemTime)>>,
}

impl MemoryCache {
    pub fn with(key: &str, value: &str, stored_at: SystemTime) -> Arc<Self> {
        let cache = Self::default();
        cache
            .entries
            .lock()
            .unwrap()
            .insert(key.into(), (value.into(), stored_at));
        Arc::new(cache)
    }

    pub fn stored(&self, key: &str) -> Option<String> {
        self.entries
            .lock()
            .unwrap()
            .get(key)
            .map(|(value, _)| value.clone())
    }
}

impl CacheStore for MemoryCache {
    fn put(&self, key: &str, value: &str, stored_at: SystemTime) -> Result<(), StorageError> {
        self.entries
            .lock()
            .unwrap()
            .insert(key.into(), (value.into(), stored_at));
        Ok(())
    }

    fn get(
        &self,
        key: &str,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<Option<CachedValue>, StorageError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(key)
            .map(|(value, stored_at)| CachedValue {
                value: value.clone(),
                stored_at: *stored_at,
                freshness: Freshness::of(*stored_at, now, ttl),
            }))
    }

    fn purge_older_than(&self, _cutoff: SystemTime) -> Result<usize, StorageError> {
        Ok(0)
    }
}
