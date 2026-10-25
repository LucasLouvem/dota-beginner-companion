//! Testes unitários de `src/storage/schema.rs`, com bancos em diretórios
//! temporários.

use super::*;
use crate::application::error::{DiagnosticCode, Redactor, ToDiagnostic};
use std::os::unix::fs::PermissionsExt;

fn user_version(connection: &Connection) -> i64 {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Banco como a versão 1 do aplicativo criava, com um valor no cache.
fn version_one_database(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE cache (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                stored_at_ms INTEGER NOT NULL
            );
            INSERT INTO cache VALUES ('heroes', '[22]', 1000);
            PRAGMA user_version = 1;",
        )
        .unwrap();
}

#[test]
fn new_database_gets_every_table_and_the_current_version() {
    let temp_dir = tempfile::tempdir().unwrap();

    let connection = open_database(&temp_dir.path().join("data/companion.sqlite3")).unwrap();

    assert_eq!(user_version(&connection), 2);
    assert_eq!(
        table_names(&connection),
        ["cache", "call_budget", "collection_cursor", "public_builds"]
    );
}

#[test]
fn version_one_database_is_migrated_and_keeps_the_cache() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    version_one_database(&path);

    let connection = open_database(&path).unwrap();

    assert_eq!(user_version(&connection), 2);
    let cached: String = connection
        .query_row("SELECT value FROM cache WHERE key = 'heroes'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(cached, "[22]");
    assert!(table_names(&connection).contains(&"public_builds".to_owned()));
}

#[test]
fn failed_migration_leaves_the_database_at_the_previous_version() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    version_one_database(&path);
    // Uma tabela com o nome de uma tabela da versão 2 faz a migração falhar
    // depois de criar `call_budget`.
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE public_builds (other TEXT);")
        .unwrap();

    let Err(storage_error) = open_database(&path) else {
        panic!("esperava falha na migração");
    };

    assert!(
        matches!(storage_error, StorageError::Open { .. }),
        "{storage_error:?}"
    );
    let connection = Connection::open(&path).unwrap();
    assert_eq!(user_version(&connection), 1);
    assert!(
        !table_names(&connection).contains(&"call_budget".to_owned()),
        "a migração desfeita não deixa tabelas novas"
    );
}

#[test]
fn reopening_keeps_rows_of_the_new_tables() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    open_database(&path)
        .unwrap()
        .execute(
            "INSERT INTO collection_cursor (id, less_than_match_id) VALUES (1, 9061960072)",
            [],
        )
        .unwrap();

    let reopened = open_database(&path).unwrap();

    let cursor: i64 = reopened
        .query_row(
            "SELECT less_than_match_id FROM collection_cursor",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cursor, 9_061_960_072);
}

#[test]
fn file_that_is_not_sqlite_is_reported_as_corrupted() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    fs::write(
        &path,
        "isto não é um banco SQLite, só texto comum com tamanho suficiente",
    )
    .unwrap();

    let Err(storage_error) = open_database(&path) else {
        panic!("esperava falha ao abrir");
    };

    assert!(
        matches!(&storage_error, StorageError::Open { path: error_path, .. } if *error_path == path),
        "{storage_error:?}"
    );
    assert_eq!(storage_error.kind(), StorageFailureKind::Corrupted);
    assert_eq!(
        storage_error.to_diagnostic(&Redactor::default()).code,
        DiagnosticCode::StorageCorrupted
    );
}

#[test]
fn read_only_directory_is_reported_as_unavailable() {
    let temp_dir = tempfile::tempdir().unwrap();
    let locked_dir = temp_dir.path().join("locked");
    fs::create_dir(&locked_dir).unwrap();
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::write(locked_dir.join("probe"), "").is_ok() {
        // Executando como root: permissões não se aplicam.
        return;
    }

    let result = open_database(&locked_dir.join("companion.sqlite3"));
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o700)).unwrap();

    let Err(storage_error) = result else {
        panic!("esperava falha ao abrir");
    };
    assert_eq!(storage_error.kind(), StorageFailureKind::Unavailable);
    assert_eq!(
        storage_error.to_diagnostic(&Redactor::default()).code,
        DiagnosticCode::StorageUnavailable
    );
}

#[test]
fn database_from_newer_app_version_is_refused_without_changes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();

    let Err(storage_error) = open_database(&path) else {
        panic!("esperava recusa do schema mais novo");
    };

    assert_eq!(storage_error.kind(), StorageFailureKind::Other);
    let diagnostic = storage_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert!(
        diagnostic
            .technical_details
            .as_deref()
            .is_some_and(|details| details.contains("versão de schema 3")),
        "{diagnostic:?}"
    );
    let connection = Connection::open(&path).unwrap();
    assert_eq!(user_version(&connection), 3);
    assert!(table_names(&connection).is_empty());
}
