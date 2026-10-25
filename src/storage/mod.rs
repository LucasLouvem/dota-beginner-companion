//! Persistência local em SQLite. Implementa os ports de armazenamento de
//! `application::ports`; o I/O é síncrono e deve rodar em `spawn_blocking`.

pub mod schema;
pub mod sqlite_cache;

use std::path::PathBuf;

/// Nome do arquivo do banco local.
pub const DATABASE_FILE_NAME: &str = "companion.sqlite3";

/// Caminho do banco no diretório de dados do usuário (XDG).
pub fn database_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "dota-companion")
        .map(|project_dirs| project_dirs.data_dir().join(DATABASE_FILE_NAME))
        .unwrap_or_else(|| PathBuf::from(DATABASE_FILE_NAME))
}
