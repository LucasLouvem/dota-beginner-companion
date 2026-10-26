//! Testes unitários de `src/application/use_cases/pro_builds.rs`, com a
//! fonte de estatísticas em memória.

use super::*;
use crate::application::ports::{
    BudgetError, BudgetExhausted, BudgetLimit, CacheStore, CallBudget,
};
use crate::domain::builds::{PlayerBuild, Purchase};
use crate::domain::match_data::MatchPlayer;
use crate::test_support::{
    FakeResponse, FakeStatsProvider, MemoryCache, MemoryPublicBuildStore, UnlimitedBudget,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::UNIX_EPOCH;

const ZEUS: u32 = 22;
const PATCH: u32 = 60;

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn candidate(match_id: u64, league_id: Option<u32>) -> HeroMatch {
    HeroMatch {
        match_id,
        // Mais recente quanto maior o ID, para a ordem do teste ser clara.
        start_time: Some(1_792_000_000 + match_id as i64),
        league_id,
        league_name: Some("Liga".into()),
        hero_won: Some(true),
        duration_secs: Some(2000),
    }
}

fn zeus_build() -> PlayerBuild {
    PlayerBuild {
        inventory: vec!["item_blink".into(), "item_refresher".into()],
        purchases: Some(vec![
            Purchase {
                item: "item_tango".into(),
                time: -89,
            },
            Purchase {
                item: "item_blink".into(),
                time: 754,
            },
        ]),
        ..PlayerBuild::default()
    }
}

fn zeus(won: bool) -> MatchPlayer {
    MatchPlayer {
        hero_id: ZEUS,
        won: Some(won),
        patch_id: Some(PATCH),
        build: Some(zeus_build()),
        ability_upgrades: Some(vec![1, 2, 1, 3, 1, 4, 1, 2, 2, 2]),
    }
}

fn details(match_id: u64, player: MatchPlayer) -> MatchDetails {
    MatchDetails {
        match_id,
        league_id: Some(100),
        game_mode: Some(22),
        patch_id: Some(PATCH),
        parsed: true,
        start_time: Some(1_792_000_000),
        players: vec![player],
    }
}

struct Setup {
    provider: Arc<FakeStatsProvider>,
    cache: Arc<MemoryCache>,
}

impl Setup {
    fn new(candidates: Vec<HeroMatch>) -> Self {
        let provider = FakeStatsProvider::default();
        provider.respond(
            &StatsQuery::HeroMatches { hero_id: ZEUS },
            FakeResponse::HeroMatches(candidates),
        );
        Self {
            provider: Arc::new(provider),
            cache: Arc::new(MemoryCache::default()),
        }
    }

    fn details(&self, details: MatchDetails) {
        self.provider.respond(
            &StatsQuery::MatchDetails {
                match_id: details.match_id,
            },
            FakeResponse::MatchDetails(details),
        );
    }

    async fn run_with(&self, budget: Arc<dyn CallBudget>, limits: ProBuildLimits) -> ProBuilds {
        let sources = ExternalSources {
            provider: self.provider.clone(),
            budget,
            cache: self.cache.clone(),
            store: Arc::new(MemoryPublicBuildStore::default()),
            embedded_patches: None,
        };
        load_pro_builds(&sources, ZEUS, PATCH, at(10_000), limits).await
    }

    async fn run(&self) -> ProBuilds {
        self.run_with(Arc::new(UnlimitedBudget), ProBuildLimits::default())
            .await
    }
}

fn match_ids(result: &ProBuilds) -> Vec<u64> {
    result
        .wins
        .iter()
        .map(|win| win.hero_match.match_id)
        .collect()
}

fn lost(match_id: u64) -> HeroMatch {
    HeroMatch {
        hero_won: Some(false),
        ..candidate(match_id, Some(100))
    }
}

#[tokio::test]
async fn the_three_most_recent_wins_are_kept_with_build_and_abilities() {
    let setup = Setup::new((1..=5).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=5 {
        setup.details(details(id, zeus(true)));
    }

    let result = setup.run().await;

    assert_eq!(match_ids(&result), [5, 4, 3]);
    assert_eq!(result.wins[0].build, zeus_build());
    assert_eq!(
        result.wins[0].ability_upgrades,
        Some(vec![1, 2, 1, 3, 1, 4, 1, 2, 2, 2])
    );
    assert_eq!(result.wins[0].hero_match, candidate(5, Some(100)));
    assert_eq!(result.calls, 4, "a lista e os 3 detalhes");
    assert_eq!(result.stopped_by, Some(ProBuildsStop::WinLimit));
    assert_eq!(result.recent_matches.len(), 5);
    assert_eq!(
        result.candidates_fetched_at,
        Some(at(10_000)),
        "lista nova, buscada agora"
    );
}

#[tokio::test]
async fn losses_in_the_list_are_skipped_without_fetching_details() {
    let setup = Setup::new(vec![
        lost(4),
        lost(3),
        candidate(2, Some(100)),
        candidate(1, Some(100)),
    ]);
    for id in 1..=2 {
        setup.details(details(id, zeus(true)));
    }

    let result = setup.run().await;

    assert_eq!(match_ids(&result), [2, 1]);
    assert_eq!(result.discarded.get(&DiscardReason::NotWon), Some(&2));
    assert_eq!(setup.provider.fetches(), 3, "a lista e as 2 vitórias");
    assert_eq!(result.stopped_by, None);
}

#[tokio::test]
async fn loss_known_only_in_the_details_is_discarded() {
    let setup = Setup::new(vec![HeroMatch {
        hero_won: None,
        ..candidate(1, Some(100))
    }]);
    setup.details(details(1, zeus(false)));

    let result = setup.run().await;

    assert!(result.wins.is_empty());
    assert_eq!(result.discarded.get(&DiscardReason::NotWon), Some(&1));
}

#[tokio::test]
async fn win_of_a_match_not_analyzed_keeps_only_the_inventory() {
    let setup = Setup::new(vec![candidate(1, Some(100))]);
    let inventory_only = PlayerBuild {
        inventory: vec!["item_blink".into()],
        ..PlayerBuild::default()
    };
    setup.details(MatchDetails {
        parsed: false,
        ..details(
            1,
            MatchPlayer {
                build: Some(inventory_only.clone()),
                ability_upgrades: None,
                ..zeus(true)
            },
        )
    });

    let result = setup.run().await;

    assert_eq!(result.wins.len(), 1);
    assert_eq!(result.wins[0].build, inventory_only);
    assert_eq!(result.wins[0].ability_upgrades, None);
}

#[tokio::test]
async fn non_league_candidates_are_discarded_without_fetching_details() {
    let setup = Setup::new(vec![candidate(1, None), candidate(2, Some(0))]);

    let result = setup.run().await;

    assert_eq!(
        result.discarded.get(&DiscardReason::NotProfessional),
        Some(&2)
    );
    assert_eq!(setup.provider.fetches(), 1, "só a lista");
}

#[tokio::test]
async fn each_invalid_match_is_discarded_with_its_reason() {
    let setup = Setup::new((1..=5).map(|id| candidate(id, Some(100))).collect());
    setup.details(MatchDetails {
        league_id: None,
        ..details(1, zeus(true))
    });
    setup.details(details(
        2,
        MatchPlayer {
            patch_id: Some(59),
            ..zeus(true)
        },
    ));
    setup.details(MatchDetails {
        patch_id: None,
        ..details(
            3,
            MatchPlayer {
                patch_id: None,
                ..zeus(true)
            },
        )
    });
    setup.details(details(
        4,
        MatchPlayer {
            build: None,
            ..zeus(true)
        },
    ));
    // A partida 5 não tem resposta: a busca falha.

    let result = setup.run().await;

    let expected = BTreeMap::from([
        (DiscardReason::NotProfessional, 1),
        (DiscardReason::OtherPatch, 1),
        (DiscardReason::UnknownPatch, 1),
        (DiscardReason::MissingPlayerData, 1),
        (DiscardReason::FetchFailed, 1),
    ]);
    assert_eq!(result.discarded, expected);
    assert!(result.wins.is_empty());
    assert!(matches!(
        result.error,
        Some(ProviderError::Unreachable { .. })
    ));
}

#[tokio::test]
async fn player_patch_wins_over_the_match_patch() {
    let setup = Setup::new(vec![candidate(1, Some(100))]);
    setup.details(MatchDetails {
        patch_id: Some(59),
        ..details(1, zeus(true))
    });

    let result = setup.run().await;

    assert_eq!(result.wins.len(), 1);
}

#[tokio::test]
async fn stops_at_the_candidate_limit() {
    let setup = Setup::new((1..=70).map(|id| candidate(id, None)).collect());

    let result = setup.run().await;

    assert_eq!(result.examined, 60);
    assert_eq!(result.stopped_by, Some(ProBuildsStop::CandidateLimit));
}

#[tokio::test]
async fn stops_at_forty_calls_counting_the_list() {
    let setup = Setup::new((1..=50).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=50 {
        setup.details(details(
            id,
            MatchPlayer {
                patch_id: Some(59),
                ..zeus(true)
            },
        ));
    }

    let result = setup.run().await;

    assert_eq!(result.calls, 40);
    assert_eq!(result.stopped_by, Some(ProBuildsStop::Budget));
    assert!(matches!(
        result.error,
        Some(ProviderError::Budget(BudgetError::Exhausted(
            BudgetExhausted {
                limit: BudgetLimit::PerQuery,
                ..
            }
        )))
    ));
    assert_eq!(result.discarded.get(&DiscardReason::OtherPatch), Some(&39));
}

/// Orçamento global que concede só as primeiras `limit` reservas.
struct LimitedBudget {
    limit: u32,
    granted: AtomicU32,
}

impl CallBudget for LimitedBudget {
    fn reserve(&self, category: CallCategory, _now: SystemTime) -> Result<(), BudgetError> {
        if self.granted.fetch_add(1, Ordering::SeqCst) >= self.limit {
            return Err(BudgetError::Exhausted(BudgetExhausted {
                category,
                limit: BudgetLimit::DailyTotal,
            }));
        }
        Ok(())
    }
}

#[tokio::test]
async fn exhausted_app_budget_returns_the_partial_result_with_the_cause() {
    let setup = Setup::new((1..=10).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=10 {
        setup.details(details(id, zeus(true)));
    }
    let budget = Arc::new(LimitedBudget {
        limit: 3,
        granted: AtomicU32::new(0),
    });

    let result = setup.run_with(budget, ProBuildLimits::default()).await;

    assert_eq!(match_ids(&result), [10, 9]);
    assert_eq!(result.stopped_by, Some(ProBuildsStop::Budget));
    assert!(matches!(
        result.error,
        Some(ProviderError::Budget(BudgetError::Exhausted(
            BudgetExhausted {
                limit: BudgetLimit::DailyTotal,
                ..
            }
        )))
    ));
}

#[tokio::test]
async fn budget_ending_on_the_list_stops_the_query() {
    let setup = Setup::new(vec![candidate(1, Some(100))]);
    let budget = Arc::new(LimitedBudget {
        limit: 0,
        granted: AtomicU32::new(0),
    });

    let result = setup.run_with(budget, ProBuildLimits::default()).await;

    assert_eq!(result.stopped_by, Some(ProBuildsStop::Budget));
    assert!(result.recent_matches.is_empty());
}

#[tokio::test]
async fn cached_details_do_not_spend_calls() {
    let setup = Setup::new((1..=3).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=3 {
        setup.details(details(id, zeus(true)));
        let key = StatsQuery::MatchDetails { match_id: id }.cache_key();
        // O fake decodifica a própria chave; o cache guarda a "resposta".
        setup.cache.put(&key, &key, at(9_000)).unwrap();
    }

    let result = setup.run().await;

    assert_eq!(result.wins.len(), 3);
    assert_eq!(result.calls, 1, "só a lista");
}
