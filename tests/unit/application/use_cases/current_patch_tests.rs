//! Testes unitários de `src/application/use_cases/current_patch.rs`.

use super::*;
use crate::application::ports::ProviderError;
use crate::application::use_cases::fallback::DataOrigin;
use crate::test_support::{FakeResponse, FakeStatsProvider, MemoryCache, UnlimitedBudget};
use std::time::UNIX_EPOCH;

fn patch(id: u32) -> Patch {
    Patch {
        id,
        name: format!("7.{id}"),
    }
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn embedded() -> Option<PatchCatalog> {
    Some(PatchCatalog::new(vec![patch(58), patch(60)]))
}

fn provider(response: Option<FakeResponse>) -> Arc<FakeStatsProvider> {
    let provider = FakeStatsProvider::default();
    if let Some(response) = response {
        provider.respond(&StatsQuery::Patches, response);
    }
    Arc::new(provider)
}

async fn load(
    provider: Arc<FakeStatsProvider>,
    cache: Arc<MemoryCache>,
    now: SystemTime,
) -> Sourced<Patch> {
    load_current_patch(provider, Arc::new(UnlimitedBudget), cache, now, embedded()).await
}

#[tokio::test]
async fn current_patch_comes_from_the_api_when_it_answers() {
    let api = provider(Some(FakeResponse::Patches(PatchCatalog::new(vec![
        patch(60),
        patch(61),
    ]))));

    let sourced = load(api, Arc::new(MemoryCache::default()), at(100)).await;

    assert_eq!(sourced.value, Some(patch(61)));
    assert_eq!(sourced.origin, Some(DataOrigin::Live));
}

#[tokio::test]
async fn malformed_answer_without_cache_falls_back_to_the_embedded_patch() {
    let api = provider(Some(FakeResponse::Malformed));

    let sourced = load(api, Arc::new(MemoryCache::default()), at(100)).await;

    assert_eq!(sourced.value, Some(patch(60)));
    assert_eq!(sourced.origin, Some(DataOrigin::Embedded));
    assert!(matches!(
        sourced.error,
        Some(ProviderError::UnexpectedFormat { .. })
    ));
}

#[tokio::test]
async fn without_cache_and_api_the_embedded_current_patch_is_used() {
    let sourced = load(provider(None), Arc::new(MemoryCache::default()), at(100)).await;

    assert_eq!(sourced.value, Some(patch(60)));
    assert_eq!(sourced.origin, Some(DataOrigin::Embedded));
}

#[tokio::test]
async fn valid_cache_is_used_without_spending_a_call() {
    let api = provider(Some(FakeResponse::Patches(PatchCatalog::new(vec![patch(
        60,
    )]))));
    let cache = Arc::new(MemoryCache::default());
    cache.put("stats/patches", "stats/patches", at(0)).unwrap();

    let sourced = load(api.clone(), cache, at(60)).await;

    assert_eq!(sourced.value, Some(patch(60)));
    assert_eq!(sourced.origin, Some(DataOrigin::Cached));
    assert_eq!(api.fetches(), 0);
}
