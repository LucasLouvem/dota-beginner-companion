//! Abertura do banco local e migrações do schema. Cada migração roda numa
//! transação e grava a versão nova em `PRAGMA user_version` dentro dela, então
//! uma falha no meio não deixa o banco pela metade.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, ErrorCode};

use crate::application::ports::{SharedError, StorageError, StorageFailureKind};

/// Versão do schema gravada em `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 3;

/// Migrações em ordem: a de índice `n` leva da versão `n` para a `n + 1`.
const MIGRATIONS: [&str; 3] = [
    // 1: cache com prazo de validade.
    "CREATE TABLE cache (
        key TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL,
        stored_at_ms INTEGER NOT NULL
    );",
    // 2: orçamento de chamadas ao OpenDota, builds de partidas públicas e o
    // ponto em que a coleta parou.
    "CREATE TABLE call_budget (
        category TEXT NOT NULL,
        period TEXT NOT NULL,
        period_key TEXT NOT NULL,
        used INTEGER NOT NULL,
        PRIMARY KEY (category, period, period_key)
    );
    CREATE TABLE public_builds (
        match_id INTEGER NOT NULL,
        hero_id INTEGER NOT NULL,
        game_mode INTEGER NOT NULL,
        patch_id INTEGER NOT NULL,
        start_time INTEGER,
        avg_rank_tier INTEGER,
        won INTEGER NOT NULL,
        evidence TEXT NOT NULL,
        items TEXT NOT NULL,
        times TEXT,
        PRIMARY KEY (match_id, hero_id)
    );
    CREATE INDEX public_builds_by_hero ON public_builds (hero_id, game_mode, patch_id);
    CREATE TABLE collection_cursor (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        less_than_match_id INTEGER NOT NULL
    );",
    // 3: as builds públicas passam a ser a build real dos vencedores. As
    // linhas da versão 2 (três itens por partida) não têm esses dados e são
    // descartadas; o cursor continua valendo.
    "DROP TABLE public_builds;
    CREATE TABLE public_wins (
        match_id INTEGER NOT NULL,
        hero_id INTEGER NOT NULL,
        game_mode INTEGER NOT NULL,
        patch_id INTEGER NOT NULL,
        start_time INTEGER,
        avg_rank_tier INTEGER,
        inventory TEXT NOT NULL,
        backpack TEXT NOT NULL,
        neutral TEXT,
        neutral_enhancement TEXT,
        purchases TEXT,
        PRIMARY KEY (match_id, hero_id)
    );
    CREATE INDEX public_wins_by_hero
        ON public_wins (hero_id, patch_id, game_mode, start_time);",
];

/// O banco foi criado por uma versão mais nova do aplicativo.
#[derive(Debug, thiserror::Error)]
#[error(
    "o banco local tem a versão de schema {found}, mais nova que a suportada ({SCHEMA_VERSION})"
)]
pub struct SchemaTooNew {
    pub found: i64,
}

/// Abre ou cria o banco, criando a pasta e aplicando as migrações que faltam.
pub fn open_database(path: &Path) -> Result<Connection, StorageError> {
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
    let mut connection = Connection::open(path).map_err(sqlite_open_error)?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sqlite_open_error)?;
    if version > SCHEMA_VERSION {
        return Err(open_error(
            StorageFailureKind::Other,
            Arc::new(SchemaTooNew { found: version }),
        ));
    }
    migrate(&mut connection, version).map_err(sqlite_open_error)?;
    Ok(connection)
}

/// Aplica, uma por transação, as migrações a partir da versão `current`.
fn migrate(connection: &mut Connection, current: i64) -> rusqlite::Result<()> {
    let first_pending = usize::try_from(current).unwrap_or(0);
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(first_pending) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", index as i64 + 1)?;
        transaction.commit()?;
    }
    Ok(())
}

/// Classifica pelo código que o próprio SQLite informa.
pub fn sqlite_kind(sqlite_error: &rusqlite::Error) -> StorageFailureKind {
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

#[cfg(test)]
#[path = "../../tests/unit/storage/schema_tests.rs"]
mod tests;
