//! Testes unitários de `src/application/use_cases/public_collection.rs`, com
//! a fonte de estatísticas e o armazenamento em memória.

use super::*;
use crate::application::ports::{BudgetError, BudgetExhausted, BudgetLimit};
use crate::domain::builds::{BuildEvidence, MatchBuild, Purchase};
use crate::domain::item_catalog::ItemInfo;
use crate::domain::match_data::MatchPlayer;
use crate::test_support::{FakeResponse, FakeStatsProvider, MemoryCache, UnlimitedBudget};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

const PATCH: u32 = 60;

/// Armazenamento em memória: guarda as builds e o histórico do cursor.
#[derive(Default)]
struct MemoryStore {
    builds: Mutex<Vec<PublicBuild>>,
    cursor: Mutex<Option<u64>>,
}

impl MemoryStore {
    fn starting_at(cursor: u64) -> Arc<Self> {
        let store = Self::default();
        *store.cursor.lock().unwrap() = Some(cursor);
        Arc::new(store)
    }

    fn saved(&self) -> Vec<PublicBuild> {
        self.builds.lock().unwrap().clone()
    }

    fn current_cursor(&self) -> Option<u64> {
        *self.cursor.lock().unwrap()
    }
}

impl PublicBuildStore for MemoryStore {
    fn save_match(&self, builds: &[PublicBuild], cursor: u64) -> Result<(), StorageError> {
        self.builds.lock().unwrap().extend_from_slice(builds);
        *self.cursor.lock().unwrap() = Some(cursor);
        Ok(())
    }

    fn cursor(&self) -> Result<Option<u64>, StorageError> {
        Ok(self.current_cursor())
    }

    fn reset_cursor(&self) -> Result<(), StorageError> {
        *self.cursor.lock().unwrap() = None;
        Ok(())
    }

    fn builds(
        &self,
        _hero_id: u32,
        _game_modes: &[u32],
        _patch_id: u32,
    ) -> Result<Vec<(MatchBuild, bool)>, StorageError> {
        Ok(Vec::new())
    }

    fn purge_other_patches(&self, _patch_id: u32) -> Result<usize, StorageError> {
        Ok(0)
    }
}

/// Orçamento que concede `on_demand` sempre e só as primeiras
/// `collection_limit` reservas da coleta.
struct CollectionBudget {
    collection_limit: u32,
    granted: AtomicU32,
}

impl CallBudget for CollectionBudget {
    fn reserve(&self, category: CallCategory, _now: SystemTime) -> Result<(), BudgetError> {
        if category == CallCategory::Collection
            && self.granted.fetch_add(1, Ordering::SeqCst) >= self.collection_limit
        {
            return Err(BudgetError::Exhausted(BudgetExhausted {
                category,
                limit: BudgetLimit::DailyCollection,
            }));
        }
        Ok(())
    }
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
    ]))
}

fn summary(match_id: u64, game_mode: u32, lobby_type: u32) -> PublicMatchSummary {
    PublicMatchSummary {
        match_id,
        game_mode: Some(game_mode),
        lobby_type: Some(lobby_type),
        avg_rank_tier: Some(51),
        start_time: Some(1_792_970_000),
    }
}

fn player(hero_id: u32, patch_id: Option<u32>) -> MatchPlayer {
    MatchPlayer {
        hero_id,
        won: Some(true),
        patch_id,
        purchase_log: None,
        final_items: Some(vec![
            "item_satanic".into(),
            "item_black_king_bar".into(),
            "item_desolator".into(),
        ]),
        ability_upgrades: None,
    }
}

fn details(match_id: u64, players: Vec<MatchPlayer>) -> MatchDetails {
    MatchDetails {
        match_id,
        league_id: None,
        game_mode: Some(23),
        patch_id: None,
        parsed: false,
        start_time: Some(1_792_970_100),
        players,
    }
}

fn listing(cursor: Option<u64>) -> StatsQuery {
    StatsQuery::PublicMatches {
        less_than_match_id: cursor,
    }
}

fn respond_details(provider: &FakeStatsProvider, match_details: MatchDetails) {
    provider.respond(
        &StatsQuery::MatchDetails {
            match_id: match_details.match_id,
        },
        FakeResponse::MatchDetails(match_details),
    );
}

fn sources(
    provider: Arc<FakeStatsProvider>,
    store: Arc<MemoryStore>,
    budget: Arc<dyn CallBudget>,
) -> CollectionSources {
    CollectionSources {
        provider,
        budget,
        store,
        cache: Arc::new(MemoryCache::default()),
        items: items(),
        embedded_patches: None,
    }
}

/// Amostra com uma partida turbo, uma contra bots, uma de outro modo, uma
/// ranqueada com jogadores de dois patches e uma turbo sem detalhes.
fn sample_provider() -> Arc<FakeStatsProvider> {
    let provider = FakeStatsProvider::default();
    provider.respond(
        &listing(None),
        FakeResponse::PublicMatches(vec![
            summary(105, 23, 0),
            summary(104, 22, 7),
            summary(103, 13, 14),
            summary(102, 1, 4),
            summary(101, 23, 0),
        ]),
    );
    respond_details(
        &provider,
        details(
            105,
            vec![
                player(22, Some(PATCH)),
                MatchPlayer {
                    final_items: None,
                    ..player(49, Some(PATCH))
                },
            ],
        ),
    );
    respond_details(
        &provider,
        details(104, vec![player(22, Some(PATCH)), player(49, Some(59))]),
    );
    // 101 não tem detalhes registrados: a busca falha.
    Arc::new(provider)
}

#[tokio::test]
async fn collects_builds_only_from_all_pick_and_turbo_between_players() {
    let provider = sample_provider();
    let store = Arc::new(MemoryStore::default());

    let outcome = collect_once(
        &sources(provider.clone(), store.clone(), Arc::new(UnlimitedBudget)),
        PATCH,
    )
    .await;

    let CycleOutcome::Collected {
        listed,
        accepted,
        builds,
        restarted,
    } = outcome
    else {
        panic!("esperava ciclo completo: {outcome:?}");
    };
    assert_eq!((listed, accepted, builds), (5, 3, 2));
    assert_eq!(
        provider.fetches(),
        4,
        "a lista e os detalhes de 105, 104 e 101"
    );
    let saved: Vec<(u64, u32, u32)> = store
        .saved()
        .iter()
        .map(|build| (build.match_id, build.hero_id, build.game_mode))
        .collect();
    assert_eq!(saved, [(105, 22, 23), (104, 22, 22)]);
    assert!(restarted, "um jogador de outro patch reinicia a coleta");
    assert_eq!(store.current_cursor(), None);
}

#[tokio::test]
async fn listing_starts_from_the_saved_cursor() {
    let provider = FakeStatsProvider::default();
    provider.respond(
        &listing(Some(500)),
        FakeResponse::PublicMatches(vec![summary(450, 23, 0)]),
    );
    respond_details(&provider, details(450, vec![player(22, Some(PATCH))]));
    let store = MemoryStore::starting_at(500);

    let outcome = collect_once(
        &sources(Arc::new(provider), store.clone(), Arc::new(UnlimitedBudget)),
        PATCH,
    )
    .await;

    assert!(
        matches!(
            outcome,
            CycleOutcome::Collected {
                builds: 1,
                restarted: false,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(store.current_cursor(), Some(450));
}

#[tokio::test]
async fn failed_listing_keeps_the_cursor() {
    let store = MemoryStore::starting_at(500);

    let outcome = collect_once(
        &sources(
            Arc::new(FakeStatsProvider::default()),
            store.clone(),
            Arc::new(UnlimitedBudget),
        ),
        PATCH,
    )
    .await;

    assert!(
        matches!(
            outcome,
            CycleOutcome::SourceFailed(ProviderError::Unreachable { .. })
        ),
        "{outcome:?}"
    );
    assert_eq!(store.current_cursor(), Some(500));
}

#[tokio::test]
async fn exhausted_collection_budget_mid_cycle_keeps_the_last_saved_match() {
    let store = Arc::new(MemoryStore::default());
    let budget = Arc::new(CollectionBudget {
        collection_limit: 2,
        granted: AtomicU32::new(0),
    });

    let outcome = collect_once(&sources(sample_provider(), store.clone(), budget), PATCH).await;

    assert!(
        matches!(outcome, CycleOutcome::BudgetExhausted),
        "{outcome:?}"
    );
    assert_eq!(store.current_cursor(), Some(105));
    assert_eq!(store.saved().len(), 1);
}

#[tokio::test]
async fn without_collection_budget_nothing_is_fetched_and_screens_still_reserve() {
    let provider = sample_provider();
    let budget = Arc::new(CollectionBudget {
        collection_limit: 0,
        granted: AtomicU32::new(0),
    });

    let outcome = collect_once(
        &sources(
            provider.clone(),
            Arc::new(MemoryStore::default()),
            budget.clone(),
        ),
        PATCH,
    )
    .await;

    assert!(
        matches!(outcome, CycleOutcome::BudgetExhausted),
        "{outcome:?}"
    );
    assert_eq!(provider.fetches(), 0);
    assert!(
        budget
            .reserve(CallCategory::OnDemand, SystemTime::now())
            .is_ok()
    );
}

#[tokio::test]
async fn empty_listing_restarts_from_the_newest_matches() {
    let provider = FakeStatsProvider::default();
    provider.respond(&listing(Some(10)), FakeResponse::PublicMatches(vec![]));
    let store = MemoryStore::starting_at(10);

    let outcome = collect_once(
        &sources(Arc::new(provider), store.clone(), Arc::new(UnlimitedBudget)),
        PATCH,
    )
    .await;

    assert!(
        matches!(
            outcome,
            CycleOutcome::Collected {
                listed: 0,
                restarted: true,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(store.current_cursor(), None);
}

#[test]
fn analyzed_player_gets_an_ordered_build_and_the_rest_an_unordered_one() {
    let analyzed = MatchPlayer {
        purchase_log: Some(vec![
            Purchase {
                item: "item_desolator".into(),
                time: 700,
            },
            Purchase {
                item: "item_black_king_bar".into(),
                time: 900,
            },
            Purchase {
                item: "item_satanic".into(),
                time: 1500,
            },
        ]),
        ..player(22, Some(PATCH))
    };
    let match_details = details(1, vec![analyzed, player(49, Some(PATCH))]);

    let (builds, other_patch) = public_builds(&match_details, &summary(1, 23, 0), &items(), PATCH);

    let evidence: Vec<BuildEvidence> = builds
        .iter()
        .map(|build| build.build.key.evidence)
        .collect();
    assert_eq!(
        evidence,
        [
            BuildEvidence::OrderedPurchaseLog,
            BuildEvidence::FinalInventoryUnordered
        ]
    );
    assert_eq!(builds[0].avg_rank_tier, Some(51));
    assert_eq!(builds[0].start_time, Some(1_792_970_100));
    assert!(!other_patch);
}

#[test]
fn players_without_patch_result_or_items_are_left_out() {
    let match_details = details(
        1,
        vec![
            player(1, None),
            MatchPlayer {
                won: None,
                ..player(2, Some(PATCH))
            },
            MatchPlayer {
                final_items: Some(vec!["item_satanic".into()]),
                ..player(3, Some(PATCH))
            },
        ],
    );

    let (builds, other_patch) = public_builds(&match_details, &summary(1, 23, 0), &items(), PATCH);

    assert!(builds.is_empty());
    assert!(!other_patch, "jogador sem patch não conta como outro patch");
}

#[test]
fn match_patch_is_used_when_the_player_has_none() {
    let match_details = MatchDetails {
        patch_id: Some(PATCH),
        ..details(1, vec![player(22, None)])
    };

    let (builds, _) = public_builds(&match_details, &summary(1, 23, 0), &items(), PATCH);

    assert_eq!(builds.len(), 1);
}
