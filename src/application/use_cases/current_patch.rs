//! Patch atual do jogo, com o fallback dos dados externos: cache válido, API,
//! cache vencido e, por fim, os patches embutidos.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::application::ports::{CacheStore, CallCategory, StatsProvider, StatsQuery};
use crate::domain::patch::{Patch, PatchCatalog};

use super::fallback::{Sourced, load_with_fallback};

/// Por quanto tempo a lista de patches do cache vale sem nova consulta.
pub const PATCH_TTL: Duration = Duration::from_secs(6 * 3600);

/// Patch atual (o de maior ID) e de onde veio a lista.
pub async fn load_current_patch(
    provider: Arc<dyn StatsProvider>,
    cache: Arc<dyn CacheStore>,
    now: SystemTime,
    embedded: Option<PatchCatalog>,
) -> Sourced<Patch> {
    let query = StatsQuery::Patches;
    let decoder = Arc::clone(&provider);
    let catalog = load_with_fallback(
        cache,
        query.cache_key(),
        PATCH_TTL,
        now,
        || async { provider.fetch(&query, CallCategory::OnDemand).await },
        move |raw| decoder.decode_patches(raw),
        embedded,
    )
    .await;
    Sourced {
        value: catalog.value.and_then(|catalog| catalog.current().cloned()),
        origin: catalog.origin,
        fetched_at: catalog.fetched_at,
        error: catalog.error,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/current_patch_tests.rs"]
mod tests;
