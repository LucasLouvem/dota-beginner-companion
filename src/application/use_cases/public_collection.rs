//! Coleta de partidas públicas em segundo plano. Cada ciclo lê uma amostra de
//! `/publicMatches` a partir do cursor, busca os detalhes das partidas de
//! Escolha Livre e Turbo entre jogadores e grava a build de cada jogador com
//! dados suficientes. A coleta só usa o orçamento da categoria dela.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::application::ports::{
    CacheStore, CallBudget, CallCategory, ProviderError, PublicBuild, PublicBuildStore,
    StatsProvider, StatsQuery, StorageError, StorageFailureKind,
};
use crate::domain::builds::build_from_match;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::{MatchDetails, PublicMatchSummary, collected_mode};
use crate::domain::patch::PatchCatalog;

use super::current_patch::load_current_patch;

/// Espera depois que o orçamento da coleta acabou; a reserva recusada não
/// usa a rede, então tentar de novo é barato.
pub const BUDGET_WAIT: Duration = Duration::from_secs(15 * 60);
/// Espera depois de uma falha da fonte ou do banco.
pub const FAILURE_WAIT: Duration = Duration::from_secs(5 * 60);
/// Pausa entre ciclos normais.
pub const CYCLE_PAUSE: Duration = Duration::from_secs(5);

/// Dependências da coleta.
pub struct CollectionSources {
    pub provider: Arc<dyn StatsProvider>,
    /// Orçamento global; a coleta reserva na categoria `Collection`.
    pub budget: Arc<dyn CallBudget>,
    pub store: Arc<dyn PublicBuildStore>,
    /// Cache usado para o patch atual.
    pub cache: Arc<dyn CacheStore>,
    pub items: Arc<ItemCatalog>,
    pub embedded_patches: Option<PatchCatalog>,
}

/// Como terminou um ciclo.
#[derive(Debug, Clone)]
pub enum CycleOutcome {
    Collected {
        /// Partidas da amostra.
        listed: usize,
        /// Partidas de Escolha Livre ou Turbo entre jogadores.
        accepted: usize,
        /// Builds gravadas.
        builds: usize,
        /// A amostra chegou a partidas de outro patch, e a coleta volta para
        /// as mais recentes.
        restarted: bool,
    },
    /// O orçamento da coleta acabou no meio; o cursor guarda o ponto.
    BudgetExhausted,
    /// A lista falhou; nada mudou.
    SourceFailed(ProviderError),
    StorageFailed(StorageError),
}

/// Builds dos jogadores de uma partida pública: só os do patch atual (do
/// jogador ou da partida), com resultado e inventário, que formam build.
/// Devolve também se algum jogador era de outro patch.
pub fn public_builds(
    details: &MatchDetails,
    summary: &PublicMatchSummary,
    items: &ItemCatalog,
    patch_id: u32,
) -> (Vec<PublicBuild>, bool) {
    let Some(game_mode) = summary.game_mode else {
        return (Vec::new(), false);
    };
    let mut other_patch = false;
    let builds = details
        .players
        .iter()
        .filter_map(|player| {
            let player_patch = details.patch_of(player)?;
            if player_patch != patch_id {
                other_patch = true;
                return None;
            }
            let input = player.build_input()?;
            let build = build_from_match(&input, items)?;
            Some(PublicBuild {
                match_id: details.match_id,
                hero_id: player.hero_id,
                game_mode,
                patch_id,
                start_time: details.start_time.or(summary.start_time),
                avg_rank_tier: summary.avg_rank_tier,
                won: input.won,
                build,
            })
        })
        .collect();
    (builds, other_patch)
}

/// Um ciclo da coleta para o patch `patch_id`.
pub async fn collect_once(sources: &CollectionSources, patch_id: u32) -> CycleOutcome {
    let store = Arc::clone(&sources.store);
    let cursor = match blocking(move || store.cursor()).await {
        Ok(cursor) => cursor,
        Err(storage_error) => return CycleOutcome::StorageFailed(storage_error),
    };
    let listing = StatsQuery::PublicMatches {
        less_than_match_id: cursor,
    };
    let summaries = match fetch(sources, &listing).await {
        Ok(raw) => match sources.provider.decode_public_matches(&raw) {
            Ok(summaries) => summaries,
            Err(provider_error) => return CycleOutcome::SourceFailed(provider_error),
        },
        Err(ProviderError::Budget(_)) => return CycleOutcome::BudgetExhausted,
        Err(provider_error) => return CycleOutcome::SourceFailed(provider_error),
    };

    let mut accepted = 0;
    let mut stored = 0;
    let mut reached_other_patch = false;
    for summary in &summaries {
        if collected_mode(summary.game_mode, summary.lobby_type).is_none() {
            continue;
        }
        accepted += 1;
        let query = StatsQuery::MatchDetails {
            match_id: summary.match_id,
        };
        let details = match fetch(sources, &query).await {
            Ok(raw) => sources.provider.decode_match_details(&raw).ok(),
            Err(ProviderError::Budget(_)) => return CycleOutcome::BudgetExhausted,
            // Partida que não veio é pulada; o cursor avança mesmo assim.
            Err(_) => None,
        };
        let (builds, other_patch) = details
            .map(|details| public_builds(&details, summary, &sources.items, patch_id))
            .unwrap_or_default();
        reached_other_patch |= other_patch;
        stored += builds.len();
        let store = Arc::clone(&sources.store);
        let match_id = summary.match_id;
        if let Err(storage_error) = blocking(move || store.save_match(&builds, match_id)).await {
            return CycleOutcome::StorageFailed(storage_error);
        }
    }

    let restarted = summaries.is_empty() || reached_other_patch;
    if restarted {
        let store = Arc::clone(&sources.store);
        if let Err(storage_error) = blocking(move || store.reset_cursor()).await {
            return CycleOutcome::StorageFailed(storage_error);
        }
    }
    CycleOutcome::Collected {
        listed: summaries.len(),
        accepted,
        builds: stored,
        restarted,
    }
}

/// Coleta enquanto o aplicativo estiver aberto: confere o patch atual,
/// apaga builds de outros patches quando ele muda e roda os ciclos, com
/// esperas depois de orçamento esgotado ou falha. Termina só com o runtime.
pub async fn run_collection(sources: CollectionSources) {
    let mut purged_for: Option<u32> = None;
    loop {
        let patch = load_current_patch(
            Arc::clone(&sources.provider),
            Arc::clone(&sources.budget),
            Arc::clone(&sources.cache),
            SystemTime::now(),
            sources.embedded_patches.clone(),
        )
        .await;
        let Some(patch) = patch.value else {
            tracing::warn!("public match collection waiting: current patch unknown");
            tokio::time::sleep(FAILURE_WAIT).await;
            continue;
        };
        if purged_for != Some(patch.id) {
            let store = Arc::clone(&sources.store);
            let patch_id = patch.id;
            match blocking(move || store.purge_other_patches(patch_id)).await {
                Ok(removed) => {
                    tracing::debug!(patch_id, removed, "removed public builds of other patches");
                    purged_for = Some(patch.id);
                }
                Err(storage_error) => {
                    tracing::warn!(error = %storage_error, "could not purge old public builds");
                }
            }
        }
        let outcome = collect_once(&sources, patch.id).await;
        tracing::debug!(?outcome, "public match collection cycle finished");
        let wait = match outcome {
            CycleOutcome::Collected { .. } => CYCLE_PAUSE,
            CycleOutcome::BudgetExhausted => BUDGET_WAIT,
            CycleOutcome::SourceFailed(_) | CycleOutcome::StorageFailed(_) => FAILURE_WAIT,
        };
        tokio::time::sleep(wait).await;
    }
}

async fn fetch(sources: &CollectionSources, query: &StatsQuery) -> Result<String, ProviderError> {
    sources
        .provider
        .fetch(query, CallCategory::Collection, sources.budget.as_ref())
        .await
}

/// Roda uma operação do banco fora da thread assíncrona. Uma tarefa
/// interrompida conta como falha do banco.
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, StorageError> + Send + 'static,
) -> Result<T, StorageError> {
    tokio::task::spawn_blocking(operation)
        .await
        .unwrap_or_else(|join_error| {
            Err(StorageError::Operation {
                operation: "acessar as builds públicas",
                kind: StorageFailureKind::Other,
                source: Arc::new(join_error),
            })
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/public_collection_tests.rs"]
mod tests;
