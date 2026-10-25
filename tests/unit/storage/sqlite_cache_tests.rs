//! Testes unitários de `src/storage/sqlite_cache.rs`, com bancos em
//! diretórios temporários.

use super::*;

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
