//! Testes unitários de `src/application/use_cases/pro_builds.rs`, com a
//! fonte de estatísticas em memória.

use super::*;
use crate::application::ports::{BudgetError, BudgetExhausted, BudgetLimit};
use crate::domain::builds::{BuildEvidence, BuildKey, Purchase};
use crate::domain::item_catalog::ItemInfo;
use crate::domain::match_data::MatchPlayer;
use crate::test_support::{FakeResponse, FakeStatsProvider, MemoryCache, UnlimitedBudget};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::UNIX_EPOCH;

const ZEUS: u32 = 22;
const PATCH: u32 = 60;

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn items() -> Arc<ItemCatalog> {
    let major = |id: u32, name: &str| ItemInfo {
        id,
        name: name.into(),
        display_name: name.into(),
        cost: Some(4000),
        components: vec!["item_component".into()],
    };
    Arc::new(ItemCatalog::new(vec![
        major(1, "item_black_king_bar"),
        major(2, "item_desolator"),
        major(3, "item_satanic"),
        major(4, "item_manta"),
    ]))
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

fn zeus(won: bool) -> MatchPlayer {
    MatchPlayer {
        hero_id: ZEUS,
        won: Some(won),
        patch_id: Some(PATCH),
        purchase_log: Some(vec![
            Purchase {
                item: "item_black_king_bar".into(),
                time: 900,
            },
            Purchase {
                item: "item_desolator".into(),
                time: 1300,
            },
            Purchase {
                item: "item_satanic".into(),
                time: 1800,
            },
        ]),
        final_items: Some(vec![]),
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
        let sources = ProBuildsSources {
            provider: self.provider.clone(),
            budget,
            cache: self.cache.clone(),
            items: items(),
        };
        load_pro_builds(&sources, ZEUS, PATCH, at(10_000), limits).await
    }

    async fn run(&self) -> ProBuilds {
        self.run_with(Arc::new(UnlimitedBudget), ProBuildLimits::default())
            .await
    }
}

fn ordered_key() -> BuildKey {
    BuildKey {
        evidence: BuildEvidence::OrderedPurchaseLog,
        items: vec![
            "item_black_king_bar".into(),
            "item_desolator".into(),
            "item_satanic".into(),
        ],
    }
}

#[tokio::test]
async fn valid_league_matches_form_the_ranking_and_ability_order() {
    let setup = Setup::new((1..=12).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=12 {
        setup.details(details(id, zeus(id <= 9)));
    }

    let result = setup.run().await;

    assert_eq!(result.valid_matches, 12);
    assert_eq!(result.ranking.analyzed, 12);
    assert_eq!(result.ranking.builds.len(), 1);
    let build = &result.ranking.builds[0];
    assert_eq!(build.key, ordered_key());
    assert_eq!((build.matches, build.wins), (12, 9));
    assert_eq!(build.average_times, Some(vec![900, 1300, 1800]));
    assert_eq!(
        result.ability_order.map(|order| order.ability_ids),
        Some(vec![1, 2, 1, 3, 1, 4, 1, 2, 2, 2])
    );
    assert_eq!(result.calls, 13, "a lista e os 12 detalhes");
    assert_eq!(result.stopped_by, None);
    assert_eq!(result.recent_matches.len(), 12);
}

#[tokio::test]
async fn fewer_than_ten_matches_of_a_build_give_insufficient_sample() {
    let setup = Setup::new((1..=9).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=9 {
        setup.details(details(id, zeus(true)));
    }

    let result = setup.run().await;

    assert_eq!(result.valid_matches, 9);
    assert!(result.ranking.builds.is_empty());
    assert_eq!(result.ability_order, None);
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
    let setup = Setup::new((1..=6).map(|id| candidate(id, Some(100))).collect());
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
    setup.details(MatchDetails {
        parsed: false,
        ..details(4, zeus(true))
    });
    setup.details(details(
        5,
        MatchPlayer {
            purchase_log: None,
            ..zeus(true)
        },
    ));
    // A partida 6 não tem resposta: a busca falha.

    let result = setup.run().await;

    let expected = BTreeMap::from([
        (DiscardReason::NotProfessional, 1),
        (DiscardReason::OtherPatch, 1),
        (DiscardReason::UnknownPatch, 1),
        (DiscardReason::NotParsed, 1),
        (DiscardReason::MissingPlayerData, 1),
        (DiscardReason::FetchFailed, 1),
    ]);
    assert_eq!(result.discarded, expected);
    assert_eq!(result.valid_matches, 0);
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

    assert_eq!(result.valid_matches, 1);
}

#[tokio::test]
async fn stops_after_thirty_valid_matches() {
    let setup = Setup::new((1..=45).map(|id| candidate(id, Some(100))).collect());
    for id in 1..=45 {
        setup.details(details(id, zeus(true)));
    }

    let result = setup.run().await;

    assert_eq!(result.valid_matches, 30);
    assert_eq!(result.stopped_by, Some(ProBuildsStop::ValidLimit));
    assert_eq!(result.calls, 31);
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
        limit: 4,
        granted: AtomicU32::new(0),
    });

    let result = setup.run_with(budget, ProBuildLimits::default()).await;

    assert_eq!(result.valid_matches, 3);
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

    assert_eq!(result.valid_matches, 3);
    assert_eq!(result.calls, 1, "só a lista");
}
