//! Testes unitários de `src/application/use_cases/recommendations.rs`.

use super::*;
use crate::application::ports::{PublicBuild, PublicBuildStore};
use crate::application::state::ModeOrigin;
use crate::application::use_cases::fallback::DataOrigin;
use crate::domain::builds::{BuildEvidence, BuildKey, MatchBuild};
use crate::domain::game::GameSnapshot;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::PopularityPhase;
use crate::domain::match_events::{MatchResult, MatchSummary};
use crate::test_support::{
    FakeResponse, FakeStatsProvider, MemoryCache, MemoryPublicBuildStore, UnlimitedBudget,
};
use std::time::UNIX_EPOCH;

const PATCH: u32 = 60;

fn picked(id: u32) -> HeroPick {
    HeroPick::Picked {
        name: format!("npc_dota_hero_{id}"),
        id: Some(id),
    }
}

fn in_match(hero: HeroPick) -> MatchStatus {
    MatchStatus {
        game: Some(GameSnapshot {
            hero,
            ..GameSnapshot::menu()
        }),
        ..MatchStatus::default()
    }
}

fn last_match(hero: HeroPick) -> MatchSummary {
    MatchSummary {
        hero,
        result: MatchResult::Victory,
        duration_secs: None,
        hero_level: None,
        stats: Default::default(),
        final_items: vec![],
        abilities: vec![],
        match_id: None,
    }
}

fn key(hero_id: u32, mode: GameMode) -> RecommendationKey {
    RecommendationKey {
        hero_id,
        mode,
        patch_id: PATCH,
    }
}

#[test]
fn key_uses_the_hero_of_the_current_match() {
    let status = MatchStatus {
        mode: ModeState::Confirmed {
            mode: GameMode::Turbo,
            origin: ModeOrigin::Chosen,
        },
        ..in_match(picked(22))
    };
    assert_eq!(
        desired_key(&status, Some(PATCH)),
        Some(key(22, GameMode::Turbo))
    );
}

#[test]
fn key_uses_the_last_match_hero_when_the_current_state_has_none() {
    let status = MatchStatus {
        last_match: Some(last_match(picked(49))),
        mode: ModeState::Awaiting {
            preselected: GameMode::Bots,
        },
        ..in_match(HeroPick::NotPicked)
    };
    assert_eq!(
        desired_key(&status, Some(PATCH)),
        Some(key(49, GameMode::Bots))
    );
}

#[test]
fn key_defaults_to_all_pick_before_any_match() {
    assert_eq!(
        desired_key(&in_match(picked(22)), Some(PATCH)),
        Some(key(22, GameMode::AllPick))
    );
}

#[test]
fn no_key_without_hero_id_or_patch() {
    assert_eq!(desired_key(&MatchStatus::default(), Some(PATCH)), None);
    let without_id = in_match(HeroPick::Picked {
        name: "npc_dota_hero_zuus".into(),
        id: None,
    });
    assert_eq!(desired_key(&without_id, Some(PATCH)), None);
    assert_eq!(desired_key(&in_match(picked(22)), None), None);
}

fn unordered(items: [&str; 3]) -> MatchBuild {
    MatchBuild {
        key: BuildKey {
            evidence: BuildEvidence::FinalInventoryUnordered,
            items: items.iter().map(|item| (*item).into()).collect(),
        },
        times: None,
    }
}

/// `count` partidas públicas do Zeus com a mesma build, no `game_mode`.
fn store_builds(
    store: &MemoryPublicBuildStore,
    first_match: u64,
    count: u64,
    game_mode: u32,
    build: MatchBuild,
) {
    for match_id in first_match..first_match + count {
        store
            .save_match(
                &[PublicBuild {
                    match_id,
                    hero_id: 22,
                    game_mode,
                    patch_id: PATCH,
                    start_time: None,
                    avg_rank_tier: None,
                    won: match_id % 2 == 0,
                    build: build.clone(),
                }],
                match_id,
            )
            .unwrap();
    }
}

fn sources(provider: FakeStatsProvider, store: MemoryPublicBuildStore) -> ExternalSources {
    ExternalSources {
        provider: Arc::new(provider),
        budget: Arc::new(UnlimitedBudget),
        cache: Arc::new(MemoryCache::default()),
        store: Arc::new(store),
        items: Arc::new(ItemCatalog::default()),
        embedded_patches: None,
    }
}

fn patch() -> Patch {
    Patch {
        id: PATCH,
        name: "7.41".into(),
    }
}

#[tokio::test]
async fn public_builds_come_from_the_mode_sources_only() {
    let store = MemoryPublicBuildStore::default();
    store_builds(&store, 1, 12, 23, unordered(["item_a", "item_b", "item_c"]));
    store_builds(
        &store,
        100,
        10,
        1,
        unordered(["item_x", "item_y", "item_z"]),
    );
    store_builds(
        &store,
        200,
        10,
        22,
        unordered(["item_x", "item_y", "item_z"]),
    );
    let sources = sources(FakeStatsProvider::default(), store);

    let turbo = load_recommendations(&sources, key(22, GameMode::Turbo), patch(), UNIX_EPOCH).await;
    let bots = load_recommendations(&sources, key(22, GameMode::Bots), patch(), UNIX_EPOCH).await;

    assert_eq!(turbo.public.analyzed, 12);
    assert_eq!(
        turbo.public.builds[0].key.items,
        ["item_a", "item_b", "item_c"]
    );
    assert_eq!(bots.public.analyzed, 20, "Escolha Livre comum e ranqueada");
    assert_eq!(
        bots.public.builds[0].key.items,
        ["item_x", "item_y", "item_z"]
    );
}

#[tokio::test]
async fn at_most_five_public_builds_are_kept() {
    let store = MemoryPublicBuildStore::default();
    for index in 0..7u64 {
        let name = format!("item_{index}");
        store_builds(
            &store,
            index * 100,
            10,
            23,
            unordered([name.as_str(), "item_y", "item_z"]),
        );
    }

    let recommendations = load_recommendations(
        &sources(FakeStatsProvider::default(), store),
        key(22, GameMode::Turbo),
        patch(),
        UNIX_EPOCH,
    )
    .await;

    assert_eq!(recommendations.public.builds.len(), PUBLIC_BUILDS_SHOWN);
    assert_eq!(recommendations.public.analyzed, 70);
}

#[tokio::test]
async fn popularity_and_professional_results_come_with_their_sources() {
    let provider = FakeStatsProvider::default();
    provider.respond(
        &StatsQuery::ItemPopularity { hero_id: 22 },
        FakeResponse::ItemPopularity(ItemPopularity {
            phases: vec![(PopularityPhase::Start, vec![("item_branches".into(), 189)])],
        }),
    );

    let recommendations = load_recommendations(
        &sources(provider, MemoryPublicBuildStore::default()),
        key(22, GameMode::AllPick),
        patch(),
        UNIX_EPOCH,
    )
    .await;

    assert_eq!(recommendations.popularity.origin, Some(DataOrigin::Live));
    assert_eq!(
        recommendations.popularity.value.unwrap().phases[0].1[0],
        ("item_branches".to_owned(), 189)
    );
    // Sem resposta para a lista de partidas: a parte profissional vem vazia,
    // com a causa.
    assert!(recommendations.pro.error.is_some());
    assert!(recommendations.pro.ranking.builds.is_empty());
    assert_eq!(recommendations.patch, patch());
}
