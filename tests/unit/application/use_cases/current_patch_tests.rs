//! Testes unitários de `src/application/use_cases/current_patch.rs`.

use super::*;
use crate::application::ports::{BoxFuture, BudgetError, ProviderError};
use crate::application::use_cases::fallback::DataOrigin;
use crate::test_support::MemoryCache;
use std::io;
use std::time::UNIX_EPOCH;

/// Provider que devolve sempre a mesma resposta e decodifica números como
/// a lista com um único patch de ID igual ao número.
struct FixedProvider {
    response: Result<&'static str, &'static str>,
}

impl StatsProvider for FixedProvider {
    fn fetch<'a>(
        &'a self,
        _query: &'a StatsQuery,
        _category: CallCategory,
        _budget: &'a dyn CallBudget,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            self.response
                .map(str::to_owned)
                .map_err(|message| ProviderError::Unreachable {
                    path: "/constants/patch".into(),
                    source: Arc::new(io::Error::other(message)),
                })
        })
    }

    fn decode_patches(&self, raw: &str) -> Result<PatchCatalog, ProviderError> {
        let id = raw.parse().map_err(|_| ProviderError::UnexpectedFormat {
            path: "/constants/patch".into(),
            detail: "não é um número",
        })?;
        Ok(PatchCatalog::new(vec![patch(id)]))
    }
}

/// Orçamento que sempre concede.
struct UnlimitedBudget;

impl CallBudget for UnlimitedBudget {
    fn reserve(&self, _category: CallCategory, _now: SystemTime) -> Result<(), BudgetError> {
        Ok(())
    }
}

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

async fn load(
    response: Result<&'static str, &'static str>,
    cache: Arc<MemoryCache>,
    now: SystemTime,
) -> Sourced<Patch> {
    load_current_patch(
        Arc::new(FixedProvider { response }),
        Arc::new(UnlimitedBudget),
        cache,
        now,
        embedded(),
    )
    .await
}

#[tokio::test]
async fn current_patch_comes_from_the_api_when_it_answers() {
    let sourced = load(Ok("61"), Arc::new(MemoryCache::default()), at(100)).await;

    assert_eq!(sourced.value, Some(patch(61)));
    assert_eq!(sourced.origin, Some(DataOrigin::Live));
}

#[tokio::test]
async fn malformed_answer_falls_back_to_the_stale_cache() {
    let cache = MemoryCache::with("stats/patches", "59", at(0));

    let sourced = load(Ok("formato novo"), cache, at(PATCH_TTL.as_secs() + 10)).await;

    assert_eq!(sourced.value, Some(patch(59)));
    assert_eq!(sourced.origin, Some(DataOrigin::Stale));
    assert!(matches!(
        sourced.error,
        Some(ProviderError::UnexpectedFormat { .. })
    ));
}

#[tokio::test]
async fn without_cache_and_api_the_embedded_current_patch_is_used() {
    let sourced = load(Err("sem rede"), Arc::new(MemoryCache::default()), at(100)).await;

    assert_eq!(sourced.value, Some(patch(60)));
    assert_eq!(sourced.origin, Some(DataOrigin::Embedded));
}
