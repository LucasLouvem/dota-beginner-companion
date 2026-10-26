//! Recomendações de itens e habilidades para um herói, modo e patch: builds
//! de partidas profissionais, builds de partidas públicas coletadas e a
//! popularidade de itens por fase.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::application::ports::{CallCategory, StatsQuery, StorageError, StorageFailureKind};
use crate::application::state::{MatchStatus, ModeState};
use crate::domain::builds::{BuildRanking, MIN_SAMPLE, rank_builds};
use crate::domain::game::{GameMode, HeroPick};
use crate::domain::match_data::{ItemPopularity, source_game_modes};
use crate::domain::patch::Patch;

use super::external_sources::ExternalSources;
use super::fallback::{Sourced, load_with_fallback};
use super::pro_builds::{ProBuildLimits, ProBuilds, load_pro_builds};
use super::recommendation_request::RecommendationKey;

/// Builds públicas mostradas.
pub const PUBLIC_BUILDS_SHOWN: usize = 5;
/// Validade da popularidade de itens no cache.
pub const POPULARITY_TTL: Duration = Duration::from_secs(6 * 3600);

/// Recomendações de uma consulta.
#[derive(Debug, Clone)]
pub struct Recommendations {
    pub key: RecommendationKey,
    pub patch: Patch,
    pub pro: ProBuilds,
    pub public: BuildRanking,
    /// Falha ao ler as builds públicas guardadas.
    pub public_error: Option<StorageError>,
    pub popularity: Sourced<ItemPopularity>,
    pub loaded_at: SystemTime,
}

/// O que as recomendações devem mostrar agora: o herói da partida atual, ou
/// o da última partida quando ela é mostrada; o modo informado (ou o
/// pré-selecionado enquanto a pergunta está aberta, ou Escolha Livre antes
/// de qualquer partida); e o patch atual. `None` sem herói ou sem patch.
pub fn desired_key(status: &MatchStatus, patch_id: Option<u32>) -> Option<RecommendationKey> {
    let hero_id = |hero: &HeroPick| match hero {
        HeroPick::Picked { id, .. } => *id,
        HeroPick::NotPicked => None,
    };
    let hero_id = status
        .game
        .as_ref()
        .and_then(|game| hero_id(&game.hero))
        .or_else(|| {
            status
                .last_match
                .as_ref()
                .and_then(|summary| hero_id(&summary.hero))
        })?;
    let mode = match status.mode {
        ModeState::Confirmed { mode, .. } => mode,
        ModeState::Awaiting { preselected } => preselected,
        ModeState::NotAsked => GameMode::default(),
    };
    Some(RecommendationKey {
        hero_id,
        mode,
        patch_id: patch_id?,
    })
}

/// Consulta as recomendações de `key`.
pub async fn load_recommendations(
    sources: &ExternalSources,
    key: RecommendationKey,
    patch: Patch,
    now: SystemTime,
) -> Recommendations {
    let pro = load_pro_builds(
        sources,
        key.hero_id,
        key.patch_id,
        now,
        ProBuildLimits::default(),
    )
    .await;

    let store = Arc::clone(&sources.store);
    let (hero_id, patch_id) = (key.hero_id, key.patch_id);
    let game_modes = source_game_modes(key.mode);
    let stored = tokio::task::spawn_blocking(move || store.builds(hero_id, game_modes, patch_id))
        .await
        .unwrap_or_else(|join_error| {
            Err(StorageError::Operation {
                operation: "ler as builds públicas",
                kind: StorageFailureKind::Other,
                source: Arc::new(join_error),
            })
        });
    let (public, public_error) = match stored {
        Ok(builds) => (rank_builds(builds, MIN_SAMPLE, PUBLIC_BUILDS_SHOWN), None),
        Err(storage_error) => (BuildRanking::default(), Some(storage_error)),
    };

    let query = StatsQuery::ItemPopularity {
        hero_id: key.hero_id,
    };
    let provider = Arc::clone(&sources.provider);
    let decoder = Arc::clone(&sources.provider);
    let budget = Arc::clone(&sources.budget);
    let popularity = load_with_fallback(
        Arc::clone(&sources.cache),
        query.cache_key(),
        POPULARITY_TTL,
        now,
        || async move {
            provider
                .fetch(&query, CallCategory::OnDemand, budget.as_ref())
                .await
        },
        move |raw| decoder.decode_item_popularity(raw),
        None,
    )
    .await;

    Recommendations {
        key,
        patch,
        pro,
        public,
        public_error,
        popularity,
        loaded_at: now,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/recommendations_tests.rs"]
mod tests;
