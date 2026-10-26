//! Testes unitários de `src/application/use_cases/recommendations.rs`.

use super::*;
use crate::application::ports::PublicBuildStore;
use crate::application::state::ModeOrigin;
use crate::application::use_cases::fallback::DataOrigin;
use crate::domain::game::GameSnapshot;
use crate::domain::match_data::PopularityPhase;
use crate::domain::match_events::{MatchResult, MatchSummary};
use crate::test_support::{
    FakeResponse, FakeStatsProvider, MemoryCache, MemoryPublicBuildStore, UnlimitedBudget,
    public_win, zeus_build,
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

/// Vitórias públicas do Zeus, uma por partida, no `game_mode`.
fn store_wins(store: &MemoryPublicBuildStore, match_ids: std::ops::Range<u64>, game_mode: u32) {
    for match_id in match_ids {
        let win = PublicWin {
            game_mode,
            start_time: Some(1_792_970_000 + match_id as i64),
            ..public_win(match_id, zeus_build())
        };
        store.save_match(&[win], match_id).unwrap();
    }
}

fn match_ids(wins: &[PublicWin]) -> Vec<u64> {
    wins.iter().map(|win| win.match_id).collect()
}

fn sources(provider: FakeStatsProvider, store: MemoryPublicBuildStore) -> ExternalSources {
    ExternalSources {
        provider: Arc::new(provider),
        budget: Arc::new(UnlimitedBudget),
        cache: Arc::new(MemoryCache::default()),
        store: Arc::new(store),
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
async fn public_wins_come_from_the_mode_sources_only() {
    let store = MemoryPublicBuildStore::default();
    store_wins(&store, 1..3, 23);
    store_wins(&store, 100..102, 1);
    store_wins(&store, 200..201, 22);
    let sources = sources(FakeStatsProvider::default(), store);

    let turbo = load_recommendations(&sources, key(22, GameMode::Turbo), patch(), UNIX_EPOCH).await;
    let bots = load_recommendations(&sources, key(22, GameMode::Bots), patch(), UNIX_EPOCH).await;

    assert_eq!(match_ids(&turbo.public_wins), [2, 1]);
    assert_eq!(
        match_ids(&bots.public_wins),
        [200, 101, 100],
        "Escolha Livre comum e ranqueada, das mais recentes"
    );
    assert!(turbo.public_error.is_none());
}

#[tokio::test]
async fn at_most_five_recent_public_wins_are_kept() {
    let store = MemoryPublicBuildStore::default();
    store_wins(&store, 0..7, 23);

    let recommendations = load_recommendations(
        &sources(FakeStatsProvider::default(), store),
        key(22, GameMode::Turbo),
        patch(),
        UNIX_EPOCH,
    )
    .await;

    assert_eq!(match_ids(&recommendations.public_wins), [6, 5, 4, 3, 2]);
    assert_eq!(recommendations.public_wins.len(), PUBLIC_WINS_SHOWN);
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
    assert!(recommendations.pro.wins.is_empty());
    assert_eq!(recommendations.patch, patch());
}
