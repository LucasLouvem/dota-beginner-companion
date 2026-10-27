//! Testes unitários de `src/providers/opendota.rs`. As respostas reais estão
//! em `tests/fixtures/opendota/`.

use super::*;
use crate::providers::http_json::HttpJsonConfig;
use crate::test_support::UnlimitedBudget;
use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use std::time::Duration;

const PATCH_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/opendota/constants_patch.json"
));

async fn provider_serving(path: &'static str, body: &'static str) -> OpenDotaProvider {
    let app = Router::new().route(path, get(move || async move { body }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = HttpJsonConfig {
        base_url: format!("http://{address}"),
        timeout: Duration::from_secs(2),
        min_interval: Duration::ZERO,
        ..HttpJsonConfig::opendota()
    };
    OpenDotaProvider::new(HttpJsonClient::new(config).unwrap(), Arc::new(items()))
}

#[test]
fn real_patch_response_gives_7_41_as_current() {
    let catalog = decode_patches(PATCH_RESPONSE).unwrap();
    assert_eq!(
        catalog.current(),
        Some(&Patch {
            id: 60,
            name: "7.41".into()
        })
    );
    assert_eq!(
        catalog.by_id(59).map(|patch| patch.name.as_str()),
        Some("7.40")
    );
}

#[test]
fn patch_entries_without_numeric_id_or_name_are_left_out() {
    let catalog =
        decode_patches(r#"[{"name": "7.40", "id": 59}, {"name": "7.41", "id": "60"}, {"id": 61}]"#)
            .unwrap();
    assert_eq!(catalog.current().map(|patch| patch.id), Some(59));
}

#[test]
fn patch_response_that_is_not_a_list_has_unexpected_format() {
    let error = decode_patches(r#"{"patches": []}"#).unwrap_err();
    assert!(
        matches!(&error, ProviderError::UnexpectedFormat { path, .. } if path == "/constants/patch"),
        "{error:?}"
    );
}

#[test]
fn patch_response_without_any_valid_patch_has_unexpected_format() {
    let error = decode_patches(r#"[{"name": "7.41"}]"#).unwrap_err();
    assert!(
        matches!(error, ProviderError::UnexpectedFormat { .. }),
        "{error:?}"
    );
}

#[test]
fn patch_response_that_is_not_json_is_invalid_json() {
    let error = decode_patches("<html>").unwrap_err();
    assert!(
        matches!(error, ProviderError::InvalidJson { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn fetches_patches_from_the_constants_path() {
    let provider = provider_serving("/constants/patch", PATCH_RESPONSE).await;

    let raw = provider
        .fetch(
            &StatsQuery::Patches,
            CallCategory::OnDemand,
            &UnlimitedBudget,
        )
        .await
        .unwrap();

    assert_eq!(
        provider
            .decode_patches(&raw)
            .unwrap()
            .current()
            .map(|patch| patch.id),
        Some(60)
    );
}

const HERO_MATCHES_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/opendota/heroes_22_matches.json"
));
const MATCH_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/opendota/match_9055224065.json"
));

fn items() -> ItemCatalog {
    crate::infrastructure::item_data::embedded_catalog().unwrap()
}

#[test]
fn real_hero_matches_are_all_league_matches_with_the_hero_result() {
    let matches = decode_hero_matches(HERO_MATCHES_RESPONSE).unwrap();

    assert_eq!(matches.len(), 100);
    assert!(matches.iter().all(HeroMatch::is_league_match));
    assert_eq!(
        matches[0],
        HeroMatch {
            kda: Some(Kda {
                kills: 3,
                deaths: 8,
                assists: 16,
            }),
            match_id: 9_055_224_065,
            start_time: Some(1_792_590_894),
            league_id: Some(18631),
            league_name: Some("护鼠宝".into()),
            hero_won: Some(true),
            duration_secs: matches[0].duration_secs,
        }
    );
    assert!(matches[0].duration_secs.is_some());
}

#[test]
fn hero_match_without_numeric_match_id_is_left_out() {
    let matches = decode_hero_matches(
        r#"[{"match_id": "123", "leagueid": 1}, {"match_id": 456, "leagueid": 0}]"#,
    )
    .unwrap();
    assert_eq!(matches.len(), 1);
    assert!(!matches[0].is_league_match());
}

#[test]
fn real_match_details_have_the_zeus_full_build_and_abilities() {
    let details = decode_match_details(MATCH_RESPONSE, &items()).unwrap();

    assert_eq!(details.match_id, 9_055_224_065);
    assert_eq!(details.league_id, Some(18631));
    assert_eq!(details.game_mode, Some(22));
    assert_eq!(details.patch_id, Some(60));
    assert!(details.parsed);
    assert_eq!(details.players.len(), 10);
    let zeus = details.player_with_hero(22).unwrap();
    assert_eq!(zeus.won, Some(true));
    assert_eq!(
        zeus.kda,
        Some(Kda {
            kills: 3,
            deaths: 8,
            assists: 16
        }),
        "o mesmo resultado da lista de partidas do herói"
    );
    assert_eq!(
        zeus.stats,
        FinalStats {
            last_hits: Some(184),
            denies: Some(3),
            gpm: Some(469),
            xpm: Some(483),
            net_worth: Some(15_044),
            observers_placed: Some(4),
            sentries_placed: Some(2),
        }
    );
    assert_eq!(details.patch_of(zeus), Some(60));
    let build = zeus.build.as_ref().unwrap();
    assert_eq!(build.inventory[0], "item_refresher");
    assert_eq!(build.inventory.len(), 5, "o espaço vazio (0) fica de fora");
    assert_eq!(build.backpack, ["item_smoke_of_deceit"]);
    assert_eq!(build.neutral.as_deref(), Some("item_partisans_brand"));
    assert_eq!(
        build.neutral_enhancement.as_deref(),
        Some("item_enhancement_greedy")
    );
    let purchases = build.purchases.as_ref().unwrap();
    assert_eq!(
        purchases.len(),
        39,
        "todas as compras, inclusive consumíveis"
    );
    assert_eq!(
        purchases[0],
        Purchase {
            item: "item_tango".into(),
            time: -89
        }
    );
    assert!(
        purchases
            .windows(2)
            .all(|pair| pair[0].time <= pair[1].time),
        "em ordem de horário"
    );
    assert_eq!(
        zeus.ability_upgrades.as_ref().unwrap()[..3],
        [5111, 641, 5111]
    );
}

#[test]
fn unknown_item_id_and_empty_slot_are_left_out_and_missing_inventory_is_no_build() {
    let details = decode_match_details(
        r#"{"match_id": 1, "players": [
            {"hero_id": 22, "item_0": 110, "item_1": 0, "item_2": 999999,
             "backpack_0": 0, "item_neutral": 0},
            {"hero_id": 49, "backpack_0": 188, "item_neutral": 1873}
        ]}"#,
        &items(),
    )
    .unwrap();

    assert_eq!(
        details.players[0].build,
        Some(PlayerBuild {
            inventory: vec!["item_refresher".to_owned()],
            ..PlayerBuild::default()
        }),
        "mochila e neutro vazios; sem registro de compras"
    );
    assert_eq!(details.players[1].build, None, "sem item_0..5");
    assert!(!details.parsed);
}

#[test]
fn purchases_are_put_in_time_order() {
    let details = decode_match_details(
        r#"{"match_id": 1, "players": [{"hero_id": 22, "item_0": 0, "purchase_log": [
            {"time": 600, "key": "blink"}, {"time": -30, "key": "tango"},
            {"time": 600, "key": "ward_observer"}, {"key": "sem_horario"}
        ]}]}"#,
        &items(),
    )
    .unwrap();

    let purchases = details.players[0].build.as_ref().unwrap().purchases.clone();
    let names: Vec<String> = purchases
        .unwrap()
        .into_iter()
        .map(|purchase| purchase.item)
        .collect();
    assert_eq!(names, ["item_tango", "item_blink", "item_ward_observer"]);
}

#[test]
fn match_details_without_match_id_or_players_have_unexpected_format() {
    for raw in [r#"{"players": []}"#, r#"{"match_id": 1}"#] {
        let error = decode_match_details(raw, &items()).unwrap_err();
        assert!(
            matches!(error, ProviderError::UnexpectedFormat { .. }),
            "{raw}: {error:?}"
        );
    }
}

#[test]
fn reduced_match_keeps_only_the_used_fields() {
    let full: Value = serde_json::from_str(
        r#"{"match_id": 1, "leagueid": 5, "chat": ["oi"], "players": [
            {"hero_id": 22, "win": 1, "account_id": 123, "personaname": "alguém", "item_0": 110}
        ]}"#,
    )
    .unwrap();

    let reduced = reduce_match(&full);

    assert_eq!(
        reduced,
        serde_json::json!({
            "match_id": 1,
            "leagueid": 5,
            "players": [{"hero_id": 22, "win": 1, "item_0": 110}]
        })
    );
}

#[tokio::test]
async fn fetched_match_details_come_reduced_for_the_cache() {
    let full = r#"{"match_id": 7, "players": [{"hero_id": 22, "account_id": 123, "name": "pro"}]}"#;
    let provider = provider_serving("/matches/7", full).await;

    let raw = provider
        .fetch(
            &StatsQuery::MatchDetails { match_id: 7 },
            CallCategory::OnDemand,
            &UnlimitedBudget,
        )
        .await
        .unwrap();

    assert!(
        !raw.contains("account_id") && !raw.contains("\"name\""),
        "{raw}"
    );
    assert_eq!(
        provider.decode_match_details(&raw).unwrap().players[0].hero_id,
        22
    );
}

const PUBLIC_MATCHES_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/opendota/public_matches.json"
));

#[test]
fn real_public_matches_have_mode_lobby_and_rank() {
    let summaries = decode_public_matches(PUBLIC_MATCHES_RESPONSE).unwrap();

    assert_eq!(summaries.len(), 100);
    assert_eq!(
        summaries[0],
        PublicMatchSummary {
            match_id: 9_062_205_691,
            game_mode: Some(1),
            lobby_type: Some(4),
            avg_rank_tier: Some(51),
            start_time: Some(1_792_970_329),
        }
    );
    let turbo = summaries
        .iter()
        .filter(|summary| summary.game_mode == Some(23))
        .count();
    assert_eq!(turbo, 81);
}

#[test]
fn public_matches_that_are_not_a_list_have_unexpected_format() {
    let error = decode_public_matches(r#"{"matches": []}"#).unwrap_err();
    assert!(
        matches!(error, ProviderError::UnexpectedFormat { .. }),
        "{error:?}"
    );
}

#[test]
fn public_matches_path_carries_the_cursor() {
    assert_eq!(
        path(&StatsQuery::PublicMatches {
            less_than_match_id: Some(9_062_205_691)
        }),
        "/publicMatches?less_than_match_id=9062205691"
    );
    assert_eq!(
        path(&StatsQuery::PublicMatches {
            less_than_match_id: None
        }),
        "/publicMatches"
    );
}

const ITEM_POPULARITY_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/opendota/heroes_22_item_popularity.json"
));

#[test]
fn real_item_popularity_has_the_most_bought_item_first_in_each_phase() {
    let popularity = decode_item_popularity(ITEM_POPULARITY_RESPONSE, &items()).unwrap();

    let top: Vec<(PopularityPhase, &str, u32)> = popularity
        .phases
        .iter()
        .map(|(phase, counts)| (*phase, counts[0].0.as_str(), counts[0].1))
        .collect();
    assert_eq!(
        top,
        [
            (PopularityPhase::Start, "item_branches", 189),
            (PopularityPhase::Early, "item_boots", 96),
            (PopularityPhase::Mid, "item_staff_of_wizardry", 116),
            (PopularityPhase::Late, "item_refresher", 49),
        ]
    );
}

#[test]
fn unknown_item_ids_and_missing_phases_leave_nothing_behind() {
    let popularity =
        decode_item_popularity(r#"{"start_game_items": {"16": 3, "999999": 9}}"#, &items())
            .unwrap();

    assert_eq!(
        popularity.phases,
        vec![
            (
                PopularityPhase::Start,
                vec![("item_branches".to_owned(), 3)]
            ),
            (PopularityPhase::Early, vec![]),
            (PopularityPhase::Mid, vec![]),
            (PopularityPhase::Late, vec![]),
        ]
    );
}

#[test]
fn item_popularity_that_is_not_an_object_has_unexpected_format() {
    let error = decode_item_popularity("[]", &items()).unwrap_err();
    assert!(
        matches!(error, ProviderError::UnexpectedFormat { .. }),
        "{error:?}"
    );
}

#[test]
fn result_needs_all_three_numbers() {
    let matches = decode_hero_matches(
        r#"[{"match_id": 1, "kills": 3, "deaths": 8},
            {"match_id": 2, "kills": 3, "deaths": 8, "assists": 16}]"#,
    )
    .unwrap();
    assert_eq!(matches[0].kda, None);
    assert_eq!(
        matches[1].kda,
        Some(Kda {
            kills: 3,
            deaths: 8,
            assists: 16
        })
    );
}

#[test]
fn reduced_match_keeps_the_result_of_each_player() {
    let reduced = reduce_match(
        &serde_json::from_str(
            r#"{"match_id": 1, "players": [{"hero_id": 22, "kills": 3, "deaths": 8, "assists": 16, "account_id": 7}]}"#,
        )
        .unwrap(),
    );
    let player = &reduced["players"][0];
    assert_eq!(player["kills"], 3);
    assert!(player.get("account_id").is_none(), "nada de conta");
}

#[test]
fn missing_totals_stay_missing_and_the_reduced_match_keeps_the_used_ones() {
    let details = decode_match_details(
        r#"{"match_id": 1, "players": [{"hero_id": 22, "last_hits": 10}]}"#,
        &items(),
    )
    .unwrap();
    let stats = details.players[0].stats;
    assert_eq!(stats.last_hits, Some(10));
    assert_eq!(stats.gpm, None);

    let reduced = reduce_match(
        &serde_json::from_str(
            r#"{"match_id": 1, "players": [{"hero_id": 22, "gold_per_min": 400, "obs_placed": 3, "account_id": 7}]}"#,
        )
        .unwrap(),
    );
    assert_eq!(reduced["players"][0]["gold_per_min"], 400);
    assert_eq!(reduced["players"][0]["obs_placed"], 3);
    assert!(reduced["players"][0].get("account_id").is_none());
}
