//! Builds e ordem de habilidades de partidas profissionais recentes de um
//! herói, no patch atual. A lista de candidatas não é documentada como
//! profissional: só contam partidas de liga (`leagueid` diferente de zero),
//! conferido também nos detalhes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::application::ports::{
    CacheStore, CallBudget, CallCategory, ProviderError, StatsProvider, StatsQuery,
};
use crate::domain::builds::{
    AbilityOrder, BuildRanking, MIN_SAMPLE, MatchBuild, ability_order, build_from_match,
    rank_builds,
};
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::{HeroMatch, MatchDetails};

use super::call_budget::QueryBudget;
use super::fallback::{DataOrigin, Sourced, load_with_fallback};

/// Validade da lista de partidas do herói no cache.
pub const HERO_MATCHES_TTL: Duration = Duration::from_secs(3600);
/// Detalhes de uma partida não mudam depois que ela termina.
pub const MATCH_DETAILS_TTL: Duration = Duration::from_secs(365 * 24 * 3600);
/// Builds profissionais mostradas.
pub const PRO_BUILDS_SHOWN: usize = 3;

/// Limites de uma consulta (política do aplicativo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProBuildLimits {
    /// Candidatas examinadas, da mais recente para a mais antiga.
    pub candidates: usize,
    /// Partidas válidas usadas na agregação.
    pub valid: usize,
    /// Chamadas da consulta, contando a lista, os detalhes, novas tentativas
    /// e respostas 429. O cache não conta.
    pub calls: u32,
}

impl Default for ProBuildLimits {
    fn default() -> Self {
        Self {
            candidates: 60,
            valid: 30,
            calls: 40,
        }
    }
}

/// Por que uma candidata não entrou na agregação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiscardReason {
    /// Sem `leagueid` (ou zero) na lista ou nos detalhes.
    NotProfessional,
    /// Patch diferente do atual.
    OtherPatch,
    /// Nem o jogador nem a partida informam o patch.
    UnknownPatch,
    /// A partida não foi analisada (sem registro de compras).
    NotParsed,
    /// O jogador com o herói falta, ou não tem resultado, compras ou habilidades.
    MissingPlayerData,
    /// A busca dos detalhes falhou (rede, resposta inválida).
    FetchFailed,
}

/// O que encerrou a consulta antes de examinar todas as candidatas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProBuildsStop {
    ValidLimit,
    CandidateLimit,
    /// Orçamento da consulta ou do aplicativo esgotado.
    Budget,
}

/// Resultado da consulta.
#[derive(Debug, Clone)]
pub struct ProBuilds {
    pub ranking: BuildRanking,
    pub ability_order: Option<AbilityOrder>,
    /// Partidas recentes do herói, para a aba de dados profissionais.
    pub recent_matches: Vec<HeroMatch>,
    pub examined: usize,
    pub valid_matches: usize,
    pub discarded: BTreeMap<DiscardReason, usize>,
    pub stopped_by: Option<ProBuildsStop>,
    /// Origem da lista de candidatas.
    pub candidates_origin: Option<DataOrigin>,
    /// Última falha da fonte, quando houve.
    pub error: Option<ProviderError>,
    pub calls: u32,
}

/// Dependências da consulta.
pub struct ProBuildsSources {
    pub provider: Arc<dyn StatsProvider>,
    pub budget: Arc<dyn CallBudget>,
    pub cache: Arc<dyn CacheStore>,
    pub items: Arc<ItemCatalog>,
}

/// Consulta as builds profissionais de `hero_id` no patch `patch_id`.
pub async fn load_pro_builds(
    sources: &ProBuildsSources,
    hero_id: u32,
    patch_id: u32,
    now: SystemTime,
    limits: ProBuildLimits,
) -> ProBuilds {
    let budget = Arc::new(QueryBudget::new(Arc::clone(&sources.budget), limits.calls));
    let candidates = fetch(
        sources,
        &budget,
        StatsQuery::HeroMatches { hero_id },
        HERO_MATCHES_TTL,
        now,
        |provider, raw| provider.decode_hero_matches(raw),
    )
    .await;
    let mut result = ProBuilds {
        ranking: BuildRanking::default(),
        ability_order: None,
        recent_matches: candidates.value.clone().unwrap_or_default(),
        examined: 0,
        valid_matches: 0,
        discarded: BTreeMap::new(),
        stopped_by: None,
        candidates_origin: candidates.origin,
        error: candidates.error,
        calls: 0,
    };
    if matches!(result.error, Some(ProviderError::Budget(_))) && candidates.value.is_none() {
        result.stopped_by = Some(ProBuildsStop::Budget);
    }
    let mut ordered = result.recent_matches.clone();
    ordered.sort_by_key(|candidate| std::cmp::Reverse(candidate.start_time));

    let mut builds = Vec::new();
    let mut upgrades = Vec::new();
    for candidate in ordered {
        if result.valid_matches >= limits.valid {
            result.stopped_by = Some(ProBuildsStop::ValidLimit);
            break;
        }
        if result.examined >= limits.candidates {
            result.stopped_by = Some(ProBuildsStop::CandidateLimit);
            break;
        }
        result.examined += 1;
        if !candidate.is_league_match() {
            discard(&mut result, DiscardReason::NotProfessional);
            continue;
        }
        let details = fetch(
            sources,
            &budget,
            StatsQuery::MatchDetails {
                match_id: candidate.match_id,
            },
            MATCH_DETAILS_TTL,
            now,
            |provider, raw| provider.decode_match_details(raw),
        )
        .await;
        let Some(details) = details.value else {
            let budget_ended = matches!(details.error, Some(ProviderError::Budget(_)));
            result.error = details.error;
            if budget_ended {
                result.stopped_by = Some(ProBuildsStop::Budget);
                break;
            }
            discard(&mut result, DiscardReason::FetchFailed);
            continue;
        };
        match validate(&details, hero_id, patch_id, &sources.items) {
            Ok((build, ability_upgrades)) => {
                result.valid_matches += 1;
                builds.extend(build);
                upgrades.push(ability_upgrades);
            }
            Err(reason) => discard(&mut result, reason),
        }
    }

    result.calls = budget.used();
    result.ranking = rank_builds(builds, MIN_SAMPLE, PRO_BUILDS_SHOWN);
    result.ability_order = ability_order(&upgrades, MIN_SAMPLE);
    tracing::debug!(
        hero_id,
        patch_id,
        examined = result.examined,
        valid = result.valid_matches,
        calls = result.calls,
        discarded = ?result.discarded,
        stopped_by = ?result.stopped_by,
        "professional builds query finished"
    );
    result
}

/// Build de uma partida e se o herói venceu.
type MatchBuildEntry = (MatchBuild, bool);

/// Confere uma partida e devolve a build (se houver três itens grandes) e a
/// ordem de habilidades do jogador com o herói.
fn validate(
    details: &MatchDetails,
    hero_id: u32,
    patch_id: u32,
    items: &ItemCatalog,
) -> Result<(Option<MatchBuildEntry>, Vec<u32>), DiscardReason> {
    if !details.is_league_match() {
        return Err(DiscardReason::NotProfessional);
    }
    let player = details
        .player_with_hero(hero_id)
        .ok_or(DiscardReason::MissingPlayerData)?;
    match details.patch_of(player) {
        None => return Err(DiscardReason::UnknownPatch),
        Some(patch) if patch != patch_id => return Err(DiscardReason::OtherPatch),
        Some(_) => {}
    }
    if !details.parsed {
        return Err(DiscardReason::NotParsed);
    }
    let input = player
        .build_input()
        .ok_or(DiscardReason::MissingPlayerData)?;
    if input.purchase_log.is_none() {
        return Err(DiscardReason::MissingPlayerData);
    }
    let upgrades = player
        .ability_upgrades
        .clone()
        .ok_or(DiscardReason::MissingPlayerData)?;
    Ok((
        build_from_match(&input, items).map(|build| (build, input.won)),
        upgrades,
    ))
}

fn discard(result: &mut ProBuilds, reason: DiscardReason) {
    *result.discarded.entry(reason).or_default() += 1;
}

/// Lê uma consulta com fallback (sem dados embutidos), reservando no
/// orçamento da consulta.
async fn fetch<T>(
    sources: &ProBuildsSources,
    budget: &Arc<QueryBudget>,
    query: StatsQuery,
    ttl: Duration,
    now: SystemTime,
    decode: impl Fn(&dyn StatsProvider, &str) -> Result<T, ProviderError>,
) -> Sourced<T> {
    let provider = Arc::clone(&sources.provider);
    let decoder = Arc::clone(&sources.provider);
    let budget = Arc::clone(budget);
    let key = query.cache_key();
    load_with_fallback(
        Arc::clone(&sources.cache),
        key,
        ttl,
        now,
        || async move {
            provider
                .fetch(&query, CallCategory::OnDemand, budget.as_ref())
                .await
        },
        move |raw| decode(decoder.as_ref(), raw),
        None,
    )
    .await
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/pro_builds_tests.rs"]
mod tests;
