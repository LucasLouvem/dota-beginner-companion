//! Testes unitários de `src/application/use_cases/public_collection.rs`, com
//! a fonte de estatísticas e o armazenamento em memória.

use super::*;
use crate::application::ports::{BudgetError, BudgetExhausted, BudgetLimit, CallBudget};
use crate::domain::builds::{PlayerBuild, Purchase};
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::MatchPlayer;
use crate::test_support::{
    FakeResponse, FakeStatsProvider, MemoryCache, MemoryPublicBuildStore, UnlimitedBudget,
};
use std::sync::atomic::{AtomicU32, Ordering};

const PATCH: u32 = 60;

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
        build: Some(PlayerBuild {
            inventory: vec![
                "item_satanic".into(),
                "item_black_king_bar".into(),
                "item_desolator".into(),
            ],
            ..PlayerBuild::default()
        }),
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
    store: Arc<MemoryPublicBuildStore>,
    budget: Arc<dyn CallBudget>,
) -> ExternalSources {
    ExternalSources {
        provider,
        budget,
        store,
        cache: Arc::new(MemoryCache::default()),
        items: Arc::new(ItemCatalog::default()),
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
                    build: None,
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
async fn collects_wins_only_from_all_pick_and_turbo_between_players() {
    let provider = sample_provider();
    let store = Arc::new(MemoryPublicBuildStore::default());

    let outcome = collect_once(
        &sources(provider.clone(), store.clone(), Arc::new(UnlimitedBudget)),
        PATCH,
    )
    .await;

    let CycleOutcome::Collected {
        listed,
        accepted,
        wins,
        restarted,
    } = outcome
    else {
        panic!("esperava ciclo completo: {outcome:?}");
    };
    assert_eq!((listed, accepted, wins), (5, 3, 2));
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
    let store = Arc::new(MemoryPublicBuildStore::starting_at(500));

    let outcome = collect_once(
        &sources(Arc::new(provider), store.clone(), Arc::new(UnlimitedBudget)),
        PATCH,
    )
    .await;

    assert!(
        matches!(
            outcome,
            CycleOutcome::Collected {
                wins: 1,
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
    let store = Arc::new(MemoryPublicBuildStore::starting_at(500));

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
    let store = Arc::new(MemoryPublicBuildStore::default());
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
            Arc::new(MemoryPublicBuildStore::default()),
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
    let store = Arc::new(MemoryPublicBuildStore::starting_at(10));

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
fn only_winners_with_a_build_are_kept_with_the_whole_build() {
    let analyzed = MatchPlayer {
        build: Some(PlayerBuild {
            inventory: vec!["item_blink".into()],
            purchases: Some(vec![Purchase {
                item: "item_tango".into(),
                time: -89,
            }]),
            ..PlayerBuild::default()
        }),
        ..player(22, Some(PATCH))
    };
    let match_details = details(
        1,
        vec![
            analyzed.clone(),
            MatchPlayer {
                won: Some(false),
                ..player(49, Some(PATCH))
            },
            MatchPlayer {
                build: None,
                ..player(50, Some(PATCH))
            },
        ],
    );

    let (wins, other_patch) = public_wins(&match_details, &summary(1, 23, 0), PATCH);

    assert_eq!(
        wins.len(),
        1,
        "o perdedor e o jogador sem build ficam de fora"
    );
    assert_eq!(wins[0].hero_id, 22);
    assert_eq!(Some(&wins[0].build), analyzed.build.as_ref());
    assert_eq!(wins[0].avg_rank_tier, Some(51));
    assert_eq!(wins[0].start_time, Some(1_792_970_100));
    assert!(!other_patch);
}

#[test]
fn players_without_patch_or_result_are_left_out() {
    let match_details = details(
        1,
        vec![
            player(1, None),
            MatchPlayer {
                won: None,
                ..player(2, Some(PATCH))
            },
            MatchPlayer {
                build: Some(PlayerBuild {
                    inventory: vec!["item_satanic".into()],
                    ..PlayerBuild::default()
                }),
                ..player(3, Some(PATCH))
            },
        ],
    );

    let (wins, other_patch) = public_wins(&match_details, &summary(1, 23, 0), PATCH);

    let heroes: Vec<u32> = wins.iter().map(|win| win.hero_id).collect();
    assert_eq!(heroes, [3], "um único item já é uma build");
    assert!(!other_patch, "jogador sem patch não conta como outro patch");
}

#[test]
fn match_patch_is_used_when_the_player_has_none() {
    let match_details = MatchDetails {
        patch_id: Some(PATCH),
        ..details(1, vec![player(22, None)])
    };

    let (wins, _) = public_wins(&match_details, &summary(1, 23, 0), PATCH);

    assert_eq!(wins.len(), 1);
}
