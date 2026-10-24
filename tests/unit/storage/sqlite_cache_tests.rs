//! Testes unitários de `src/storage/sqlite_cache.rs`, com bancos em
//! diretórios temporários.

use super::*;
use crate::application::error::{DiagnosticCode, Redactor, ToDiagnostic};
use std::os::unix::fs::PermissionsExt;

const HOUR: Duration = Duration::from_secs(3600);

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn temp_store() -> (tempfile::TempDir, SqliteCacheStore) {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = SqliteCacheStore::open(&temp_dir.path().join("data/cache.sqlite3")).unwrap();
    (temp_dir, store)
}

#[test]
fn reads_back_written_value_as_fresh_within_ttl() {
    let (_temp_dir, store) = temp_store();
    store.put("heroes", "[1,2,3]", at(1_000)).unwrap();

    assert_eq!(
        store.get("heroes", HOUR, at(1_000 + 60)).unwrap(),
        Some(CachedValue {
            value: "[1,2,3]".into(),
            stored_at: at(1_000),
            freshness: Freshness::Fresh,
        })
    );
}

#[test]
fn value_past_ttl_is_still_returned_but_marked_stale() {
    let (_temp_dir, store) = temp_store();
    store.put("heroes", "[1]", at(1_000)).unwrap();

    let cached = store
        .get("heroes", HOUR, at(1_000 + 3_601))
        .unwrap()
        .expect("o valor vencido continua no cache");

    assert_eq!(cached.value, "[1]");
    assert_eq!(cached.freshness, Freshness::Stale);
}

#[test]
fn missing_key_is_none() {
    let (_temp_dir, store) = temp_store();
    assert_eq!(store.get("ausente", HOUR, at(1)).unwrap(), None);
}

#[test]
fn writing_same_key_replaces_value_and_time() {
    let (_temp_dir, store) = temp_store();
    store.put("patch", "7.37", at(10)).unwrap();
    store.put("patch", "7.38", at(20)).unwrap();

    let cached = store.get("patch", HOUR, at(20)).unwrap().unwrap();
    assert_eq!(cached.value, "7.38");
    assert_eq!(cached.stored_at, at(20));
}

#[test]
fn purge_removes_only_values_written_before_cutoff() {
    let (_temp_dir, store) = temp_store();
    store.put("old", "a", at(10)).unwrap();
    store.put("older", "b", at(5)).unwrap();
    store.put("new", "c", at(100)).unwrap();

    assert_eq!(store.purge_older_than(at(50)).unwrap(), 2);

    assert_eq!(store.get("old", HOUR, at(100)).unwrap(), None);
    assert_eq!(store.get("older", HOUR, at(100)).unwrap(), None);
    assert_eq!(
        store
            .get("new", HOUR, at(100))
            .unwrap()
            .map(|cached| cached.value),
        Some("c".into())
    );
}

#[test]
fn data_survives_reopening_the_database() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("cache.sqlite3");
    SqliteCacheStore::open(&path)
        .unwrap()
        .put("heroes", "[22]", at(1))
        .unwrap();

    let reopened = SqliteCacheStore::open(&path).unwrap();

    assert_eq!(
        reopened
            .get("heroes", HOUR, at(2))
            .unwrap()
            .map(|cached| cached.value),
        Some("[22]".into())
    );
}

#[test]
fn file_that_is_not_sqlite_is_reported_as_corrupted() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("cache.sqlite3");
    fs::write(
        &path,
        "isto não é um banco SQLite, só texto comum com tamanho suficiente",
    )
    .unwrap();

    let Err(storage_error) = SqliteCacheStore::open(&path) else {
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

    let result = SqliteCacheStore::open(&locked_dir.join("cache.sqlite3"));
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
fn database_from_newer_app_version_is_refused() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("cache.sqlite3");
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();

    let Err(storage_error) = SqliteCacheStore::open(&path) else {
        panic!("esperava recusa do schema mais novo");
    };

    assert_eq!(storage_error.kind(), StorageFailureKind::Other);
    let diagnostic = storage_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert!(
        diagnostic
            .technical_details
            .as_deref()
            .is_some_and(|details| details.contains("versão de schema 2")),
        "{diagnostic:?}"
    );
}

#[test]
fn new_database_gets_current_schema_version() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("cache.sqlite3");
    SqliteCacheStore::open(&path).unwrap();

    let version: i64 = Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}
