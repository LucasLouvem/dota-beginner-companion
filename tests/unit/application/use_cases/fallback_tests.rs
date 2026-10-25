//! Testes unitários de `src/application/use_cases/fallback.rs`.

use super::*;
use crate::application::ports::{StorageError, StorageFailureKind};
use std::collections::HashMap;
use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::UNIX_EPOCH;

const TTL: Duration = Duration::from_secs(3600);

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

/// Cache em memória com o mesmo comportamento de validade do SQLite.
#[derive(Default)]
struct MemoryCache {
    entries: Mutex<HashMap<String, (String, SystemTime)>>,
}

impl MemoryCache {
    fn with(key: &str, value: &str, stored_at: SystemTime) -> Arc<Self> {
        let cache = Self::default();
        cache
            .entries
            .lock()
            .unwrap()
            .insert(key.into(), (value.into(), stored_at));
        Arc::new(cache)
    }

    fn stored(&self, key: &str) -> Option<String> {
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

/// Cache cujo banco falha em toda operação.
struct BrokenCache;

impl CacheStore for BrokenCache {
    fn put(&self, _key: &str, _value: &str, _stored_at: SystemTime) -> Result<(), StorageError> {
        Err(broken())
    }

    fn get(
        &self,
        _key: &str,
        _ttl: Duration,
        _now: SystemTime,
    ) -> Result<Option<CachedValue>, StorageError> {
        Err(broken())
    }

    fn purge_older_than(&self, _cutoff: SystemTime) -> Result<usize, StorageError> {
        Err(broken())
    }
}

fn broken() -> StorageError {
    StorageError::Operation {
        operation: "ler o cache",
        kind: StorageFailureKind::Corrupted,
        source: Arc::new(io::Error::other("banco danificado")),
    }
}

fn offline() -> ProviderError {
    ProviderError::Unreachable {
        path: "/constants/patch".into(),
        source: Arc::new(io::Error::other("sem rede")),
    }
}

/// Decodifica números; qualquer outro texto é resposta inesperada.
fn decode(raw: &str) -> Result<u32, ProviderError> {
    raw.trim()
        .parse()
        .map_err(|_| ProviderError::UnexpectedStatus {
            path: "/constants/patch".into(),
            status: 200,
        })
}

async fn load(
    cache: Arc<dyn CacheStore>,
    now: SystemTime,
    response: Result<&str, ProviderError>,
    embedded: Option<u32>,
    calls: &AtomicU32,
) -> Sourced<u32> {
    load_with_fallback(
        cache,
        "patch".into(),
        TTL,
        now,
        || async {
            calls.fetch_add(1, Ordering::SeqCst);
            response.map(str::to_owned)
        },
        decode,
        embedded,
    )
    .await
}

#[tokio::test]
async fn valid_cache_is_used_without_calling_the_api() {
    let cache = MemoryCache::with("patch", "60", at(1000));
    let calls = AtomicU32::new(0);

    let sourced = load(cache, at(1000 + 60), Ok("61"), None, &calls).await;

    assert_eq!(sourced.value, Some(60));
    assert_eq!(sourced.origin, Some(DataOrigin::Cached));
    assert_eq!(sourced.fetched_at, Some(at(1000)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn without_cache_the_api_answer_is_used_and_cached() {
    let cache = Arc::new(MemoryCache::default());
    let calls = AtomicU32::new(0);

    let sourced = load(cache.clone(), at(5000), Ok("60"), None, &calls).await;

    assert_eq!(sourced.value, Some(60));
    assert_eq!(sourced.origin, Some(DataOrigin::Live));
    assert_eq!(sourced.fetched_at, Some(at(5000)));
    assert!(sourced.error.is_none());
    assert_eq!(cache.stored("patch").as_deref(), Some("60"));
}

#[tokio::test]
async fn stale_cache_is_replaced_by_a_new_api_answer() {
    let cache = MemoryCache::with("patch", "59", at(0));
    let calls = AtomicU32::new(0);

    let sourced = load(cache.clone(), at(10_000), Ok("60"), None, &calls).await;

    assert_eq!(sourced.value, Some(60));
    assert_eq!(sourced.origin, Some(DataOrigin::Live));
    assert_eq!(cache.stored("patch").as_deref(), Some("60"));
}

#[tokio::test]
async fn stale_cache_is_shown_as_old_when_the_api_fails() {
    let cache = MemoryCache::with("patch", "59", at(0));
    let calls = AtomicU32::new(0);

    let sourced = load(cache, at(10_000), Err(offline()), Some(58), &calls).await;

    assert_eq!(sourced.value, Some(59));
    assert_eq!(sourced.origin, Some(DataOrigin::Stale));
    assert_eq!(sourced.fetched_at, Some(at(0)));
    assert!(matches!(
        sourced.error,
        Some(ProviderError::Unreachable { .. })
    ));
}

#[tokio::test]
async fn embedded_data_is_used_without_cache_when_the_api_fails() {
    let calls = AtomicU32::new(0);

    let sourced = load(
        Arc::new(MemoryCache::default()),
        at(10),
        Err(offline()),
        Some(58),
        &calls,
    )
    .await;

    assert_eq!(sourced.value, Some(58));
    assert_eq!(sourced.origin, Some(DataOrigin::Embedded));
    assert_eq!(sourced.fetched_at, None);
}

#[tokio::test]
async fn without_any_source_there_is_no_data_and_the_cause_is_kept() {
    let calls = AtomicU32::new(0);

    let sourced = load(
        Arc::new(MemoryCache::default()),
        at(10),
        Err(offline()),
        None,
        &calls,
    )
    .await;

    assert_eq!(sourced.value, None);
    assert_eq!(sourced.origin, None);
    assert!(matches!(
        sourced.error,
        Some(ProviderError::Unreachable { .. })
    ));
}

#[tokio::test]
async fn answer_that_does_not_decode_is_not_cached_and_falls_back() {
    let cache = MemoryCache::with("patch", "59", at(0));
    let calls = AtomicU32::new(0);

    let sourced = load(cache.clone(), at(10_000), Ok("formato novo"), None, &calls).await;

    assert_eq!(sourced.value, Some(59));
    assert_eq!(sourced.origin, Some(DataOrigin::Stale));
    assert!(matches!(
        sourced.error,
        Some(ProviderError::UnexpectedStatus { .. })
    ));
    assert_eq!(cache.stored("patch").as_deref(), Some("59"));
}

#[tokio::test]
async fn broken_cache_does_not_prevent_the_api_answer() {
    let calls = AtomicU32::new(0);

    let sourced = load(Arc::new(BrokenCache), at(10), Ok("60"), None, &calls).await;

    assert_eq!(sourced.value, Some(60));
    assert_eq!(sourced.origin, Some(DataOrigin::Live));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
