//! Cache com prazo de validade gravado em SQLite.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};

use crate::application::ports::{
    CacheStore, CachedValue, Freshness, SharedError, StorageError, StorageFailureKind,
};

/// Versão do schema gravada em `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 1;

/// Migração da versão 0 (banco vazio) para a 1. Roda numa transação.
const MIGRATION_TO_V1: &str = "BEGIN;
CREATE TABLE cache (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    stored_at_ms INTEGER NOT NULL
);
PRAGMA user_version = 1;
COMMIT;";

/// O banco foi criado por uma versão mais nova do aplicativo.
#[derive(Debug, thiserror::Error)]
#[error(
    "o banco local tem a versão de schema {found}, mais nova que a suportada ({SCHEMA_VERSION})"
)]
pub struct SchemaTooNew {
    pub found: i64,
}

/// Cache em SQLite. A conexão fica num `Mutex` porque o `Connection` do
/// rusqlite pode mudar de thread (`Send`), mas não ser usado por duas ao mesmo
/// tempo (não é `Sync`).
pub struct SqliteCacheStore {
    connection: Mutex<Connection>,
}

impl SqliteCacheStore {
    /// Abre ou cria o banco, criando a pasta e aplicando as migrações.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let open_error = |kind, source: SharedError| StorageError::Open {
            path: path.to_path_buf(),
            kind,
            source,
        };
        let sqlite_open_error = |sqlite_error: rusqlite::Error| {
            open_error(sqlite_kind(&sqlite_error), Arc::new(sqlite_error))
        };
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            fs::create_dir_all(dir)
                .map_err(|io_error| open_error(io_kind(&io_error), Arc::new(io_error)))?;
        }
        let connection = Connection::open(path).map_err(sqlite_open_error)?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(sqlite_open_error)?;
        if version > SCHEMA_VERSION {
            return Err(open_error(
                StorageFailureKind::Other,
                Arc::new(SchemaTooNew { found: version }),
            ));
        }
        if version < 1 {
            connection
                .execute_batch(MIGRATION_TO_V1)
                .map_err(sqlite_open_error)?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
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

/// Classifica pelo código que o próprio SQLite informa.
fn sqlite_kind(sqlite_error: &rusqlite::Error) -> StorageFailureKind {
    match sqlite_error.sqlite_error_code() {
        Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => {
            StorageFailureKind::Unavailable
        }
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StorageFailureKind::Corrupted,
        _ => StorageFailureKind::Other,
    }
}

fn io_kind(io_error: &io::Error) -> StorageFailureKind {
    match io_error.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            StorageFailureKind::Unavailable
        }
        _ => StorageFailureKind::Other,
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
