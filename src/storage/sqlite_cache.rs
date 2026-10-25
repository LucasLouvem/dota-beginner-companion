//! Cache com prazo de validade gravado em SQLite.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::application::ports::{CacheStore, CachedValue, Freshness, StorageError};

use super::schema::{open_database, sqlite_kind};

/// Cache em SQLite. A conexão fica num `Mutex` porque o `Connection` do
/// rusqlite pode mudar de thread (`Send`), mas não ser usado por duas ao mesmo
/// tempo (não é `Sync`).
pub struct SqliteCacheStore {
    connection: Mutex<Connection>,
}

impl SqliteCacheStore {
    /// Abre ou cria o banco, criando a pasta e aplicando as migrações.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            connection: Mutex::new(open_database(path)?),
        })
    }

    /// Uma falha no meio de outra operação não deixa o SQLite inconsistente
    /// (cada comando é atômico), então a conexão continua utilizável.
    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl CacheStore for SqliteCacheStore {
    fn put(&self, key: &str, value: &str, stored_at: SystemTime) -> Result<(), StorageError> {
        self.connection()
            .execute(
                "INSERT INTO cache (key, value, stored_at_ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                                stored_at_ms = excluded.stored_at_ms",
                params![key, value, to_unix_ms(stored_at)],
            )
            .map(|_| ())
            .map_err(|sqlite_error| operation_error("gravar no cache", sqlite_error))
    }

    fn get(
        &self,
        key: &str,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<Option<CachedValue>, StorageError> {
        let row: Option<(String, i64)> = self
            .connection()
            .query_row(
                "SELECT value, stored_at_ms FROM cache WHERE key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|sqlite_error| operation_error("ler o cache", sqlite_error))?;
        Ok(row.map(|(value, stored_at_ms)| {
            let stored_at = from_unix_ms(stored_at_ms);
            CachedValue {
                value,
                stored_at,
                freshness: Freshness::of(stored_at, now, ttl),
            }
        }))
    }

    fn purge_older_than(&self, cutoff: SystemTime) -> Result<usize, StorageError> {
        self.connection()
            .execute(
                "DELETE FROM cache WHERE stored_at_ms < ?1",
                params![to_unix_ms(cutoff)],
            )
            .map_err(|sqlite_error| operation_error("limpar o cache", sqlite_error))
    }
}

fn operation_error(operation: &'static str, sqlite_error: rusqlite::Error) -> StorageError {
    StorageError::Operation {
        operation,
        kind: sqlite_kind(&sqlite_error),
        source: Arc::new(sqlite_error),
    }
}

/// Milissegundos desde 1970; horários anteriores contam como 0.
fn to_unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn from_unix_ms(milliseconds: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(u64::try_from(milliseconds).unwrap_or(0))
}

#[cfg(test)]
#[path = "../../tests/unit/storage/sqlite_cache_tests.rs"]
mod tests;
