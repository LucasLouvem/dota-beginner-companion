//! Vitórias profissionais recentes de um herói no patch atual, com a build
//! real e a ordem de habilidades de cada uma. A lista de candidatas não é
//! documentada como profissional: só contam partidas de liga (`leagueid`
//! diferente de zero), conferido também nos detalhes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::application::ports::{CallCategory, ProviderError, StatsProvider, StatsQuery};
use crate::domain::builds::PlayerBuild;
use crate::domain::match_data::{FinalStats, HeroMatch, MatchDetails};

use super::call_budget::QueryBudget;
use super::external_sources::ExternalSources;
use super::fallback::{DataOrigin, Sourced, load_with_fallback};

/// Validade da lista de partidas do herói no cache.
pub const HERO_MATCHES_TTL: Duration = Duration::from_secs(3600);
/// Detalhes de uma partida não mudam depois que ela termina.
pub const MATCH_DETAILS_TTL: Duration = Duration::from_secs(365 * 24 * 3600);
/// Vitórias profissionais mostradas.
pub const PRO_WINS_SHOWN: usize = 3;

/// Limites de uma consulta (política do aplicativo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProBuildLimits {
    /// Candidatas examinadas, da mais recente para a mais antiga.
    pub candidates: usize,
    /// Vitórias válidas procuradas.
    pub wins: usize,
    /// Chamadas da consulta, contando a lista, os detalhes, novas tentativas
    /// e respostas 429. O cache não conta.
    pub calls: u32,
}

impl Default for ProBuildLimits {
    fn default() -> Self {
        Self {
            candidates: 60,
            wins: PRO_WINS_SHOWN,
            calls: 40,
        }
    }
}

/// Por que uma candidata não entrou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiscardReason {
    /// Sem `leagueid` (ou zero) na lista ou nos detalhes.
    NotProfessional,
    /// O time do herói perdeu (ou o resultado não é conhecido).
    NotWon,
    /// Patch diferente do atual.
    OtherPatch,
    /// Nem o jogador nem a partida informam o patch.
    UnknownPatch,
    /// O jogador com o herói falta ou não tem inventário.
    MissingPlayerData,
    /// A busca dos detalhes falhou (rede, resposta inválida).
    FetchFailed,
}

/// O que encerrou a consulta antes de examinar todas as candidatas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProBuildsStop {
    WinLimit,
    CandidateLimit,
    /// Orçamento da consulta ou do aplicativo esgotado.
    Budget,
}

/// Uma vitória profissional do herói, como o jogador jogou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProWin {
    /// A partida na lista de candidatas (liga, data e duração).
    pub hero_match: HeroMatch,
    pub build: PlayerBuild,
    /// IDs das habilidades na ordem em que foram evoluídas; ausente em
    /// partida não analisada.
    pub ability_upgrades: Option<Vec<u32>>,
    /// Totais finais do jogador na partida.
    pub stats: FinalStats,
}

/// Resultado da consulta.
#[derive(Debug, Clone)]
pub struct ProBuilds {
    /// Vitórias válidas, da mais recente para a mais antiga.
    pub wins: Vec<ProWin>,
    /// Partidas recentes do herói, para a aba de dados profissionais.
    pub recent_matches: Vec<HeroMatch>,
    pub examined: usize,
    pub discarded: BTreeMap<DiscardReason, usize>,
    pub stopped_by: Option<ProBuildsStop>,
    /// Origem da lista de candidatas.
    pub candidates_origin: Option<DataOrigin>,
    /// Quando a lista de candidatas foi obtida do OpenDota.
    pub candidates_fetched_at: Option<SystemTime>,
    /// Última falha da fonte, quando houve.
    pub error: Option<ProviderError>,
    pub calls: u32,
}

/// Procura as vitórias profissionais mais recentes de `hero_id` no patch
/// `patch_id`.
pub async fn load_pro_builds(
    sources: &ExternalSources,
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
        wins: Vec::new(),
        recent_matches: candidates.value.clone().unwrap_or_default(),
        examined: 0,
        discarded: BTreeMap::new(),
        stopped_by: None,
        candidates_origin: candidates.origin,
        candidates_fetched_at: candidates.fetched_at,
        error: candidates.error,
        calls: 0,
    };
    if matches!(result.error, Some(ProviderError::Budget(_))) && candidates.value.is_none() {
        result.stopped_by = Some(ProBuildsStop::Budget);
    }
    let mut ordered = result.recent_matches.clone();
    ordered.sort_by_key(|candidate| std::cmp::Reverse(candidate.start_time));

    for candidate in ordered {
        if result.wins.len() >= limits.wins {
            result.stopped_by = Some(ProBuildsStop::WinLimit);
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
        // Derrota já conhecida pela lista não gasta a busca dos detalhes.
        if candidate.hero_won == Some(false) {
            discard(&mut result, DiscardReason::NotWon);
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
        match validate(&details, &candidate, hero_id, patch_id) {
            Ok((build, ability_upgrades, stats)) => result.wins.push(ProWin {
                hero_match: candidate,
                build,
                ability_upgrades,
                stats,
            }),
            Err(reason) => discard(&mut result, reason),
        }
    }

    result.calls = budget.used();
    tracing::debug!(
        hero_id,
        patch_id,
        examined = result.examined,
        wins = result.wins.len(),
        calls = result.calls,
        discarded = ?result.discarded,
        stopped_by = ?result.stopped_by,
        "professional wins query finished"
    );
    result
}

/// Confere uma partida e devolve a build, a ordem de habilidades e os totais
/// finais do jogador com o herói, quando é uma vitória de liga no patch.
fn validate(
    details: &MatchDetails,
    candidate: &HeroMatch,
    hero_id: u32,
    patch_id: u32,
) -> Result<(PlayerBuild, Option<Vec<u32>>, FinalStats), DiscardReason> {
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
    if player.won.or(candidate.hero_won) != Some(true) {
        return Err(DiscardReason::NotWon);
    }
    let build = player
        .build
        .clone()
        .ok_or(DiscardReason::MissingPlayerData)?;
    Ok((build, player.ability_upgrades.clone(), player.stats))
}

fn discard(result: &mut ProBuilds, reason: DiscardReason) {
    *result.discarded.entry(reason).or_default() += 1;
}

/// Lê uma consulta com fallback (sem dados embutidos), reservando no
/// orçamento da consulta.
async fn fetch<T>(
    sources: &ExternalSources,
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
