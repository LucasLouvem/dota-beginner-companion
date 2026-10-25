//! Testes unitários de `src/domain/builds.rs`. Os dados de exemplo são
//! escritos à mão.

use super::*;
use crate::domain::item_catalog::ItemInfo;

/// Itens grandes (receita e custo ≥ 2000) e componentes, como no catálogo.
fn catalog() -> ItemCatalog {
    let item = |name: &str, cost: u32, components: &[&str]| ItemInfo {
        id: 0,
        name: name.into(),
        display_name: name.into(),
        cost: Some(cost),
        components: components
            .iter()
            .map(|component| (*component).into())
            .collect(),
    };
    ItemCatalog::new(vec![
        item("item_ogre_axe", 1000, &[]),
        item("item_mithril_hammer", 1600, &[]),
        item(
            "item_black_king_bar",
            4050,
            &["item_ogre_axe", "item_mithril_hammer"],
        ),
        item("item_blink", 2250, &[]),
        item(
            "item_desolator",
            3500,
            &["item_mithril_hammer", "item_blight_stone"],
        ),
        item("item_satanic", 5050, &["item_reaver", "item_claymore"]),
        item("item_manta", 4650, &["item_yasha", "item_ultimate_orb"]),
        item("item_power_treads", 1400, &["item_boots", "item_gloves"]),
    ])
}

fn buy(item: &str, time: i32) -> Purchase {
    Purchase {
        item: item.into(),
        time,
    }
}

fn analyzed(won: bool, log: Vec<Purchase>) -> PlayerMatch {
    PlayerMatch {
        won,
        purchase_log: Some(log),
        final_items: vec![],
    }
}

fn not_analyzed(won: bool, final_items: &[&str]) -> PlayerMatch {
    PlayerMatch {
        won,
        purchase_log: None,
        final_items: final_items.iter().map(|item| (*item).into()).collect(),
    }
}

fn ordered(items: &[&str]) -> BuildKey {
    BuildKey {
        evidence: BuildEvidence::OrderedPurchaseLog,
        items: items.iter().map(|item| (*item).into()).collect(),
    }
}

fn unordered(items: &[&str]) -> BuildKey {
    BuildKey {
        evidence: BuildEvidence::FinalInventoryUnordered,
        items: items.iter().map(|item| (*item).into()).collect(),
    }
}

#[test]
fn analyzed_match_build_follows_first_purchase_time_without_components() {
    let player = analyzed(
        true,
        vec![
            buy("item_ogre_axe", 500),
            buy("item_desolator", 1300),
            buy("item_mithril_hammer", 700),
            buy("item_black_king_bar", 900),
            buy("item_satanic", 1800),
            buy("item_manta", 2400),
        ],
    );

    let build = build_from_match(&player, &catalog()).unwrap();

    assert_eq!(
        build,
        MatchBuild {
            key: ordered(&["item_black_king_bar", "item_desolator", "item_satanic"]),
            times: Some(vec![900, 1300, 1800]),
        }
    );
}

#[test]
fn repeated_purchase_counts_once_at_its_first_time() {
    let player = analyzed(
        false,
        vec![
            buy("item_black_king_bar", 900),
            buy("item_black_king_bar", 1500),
            buy("item_manta", 1000),
            buy("item_desolator", 1600),
        ],
    );

    let build = build_from_match(&player, &catalog()).unwrap();

    assert_eq!(
        build.key,
        ordered(&["item_black_king_bar", "item_manta", "item_desolator"])
    );
    assert_eq!(build.times, Some(vec![900, 1000, 1600]));
}

#[test]
fn analyzed_match_with_fewer_than_three_major_items_has_no_build() {
    let player = analyzed(
        true,
        vec![
            buy("item_power_treads", 600),
            buy("item_black_king_bar", 900),
            buy("item_blink", 1000),
        ],
    );
    assert_eq!(build_from_match(&player, &catalog()), None);
}

#[test]
fn match_without_purchase_log_uses_final_inventory_as_unordered_set() {
    let player = not_analyzed(
        true,
        &[
            "item_power_treads",
            "item_satanic",
            "item_black_king_bar",
            "item_manta",
            "item_blink",
        ],
    );

    let build = build_from_match(&player, &catalog()).unwrap();

    assert_eq!(
        build,
        MatchBuild {
            key: unordered(&["item_black_king_bar", "item_manta", "item_satanic"]),
            times: None,
        }
    );
}

#[test]
fn empty_purchase_log_falls_back_to_the_final_inventory() {
    let player = PlayerMatch {
        won: true,
        purchase_log: Some(vec![]),
        final_items: vec![
            "item_satanic".into(),
            "item_desolator".into(),
            "item_manta".into(),
        ],
    };
    assert_eq!(
        build_from_match(&player, &catalog()).unwrap().key,
        unordered(&["item_desolator", "item_manta", "item_satanic"])
    );
}

#[test]
fn final_inventory_with_fewer_than_three_major_items_has_no_build() {
    let player = not_analyzed(
        false,
        &["item_satanic", "item_power_treads", "item_ogre_axe"],
    );
    assert_eq!(build_from_match(&player, &catalog()), None);
}

#[test]
fn same_items_with_different_evidence_are_different_builds() {
    assert_ne!(
        ordered(&["item_black_king_bar", "item_manta", "item_satanic"]),
        unordered(&["item_black_king_bar", "item_manta", "item_satanic"])
    );
}

fn build(key: BuildKey, times: Option<Vec<i32>>) -> MatchBuild {
    MatchBuild { key, times }
}

/// `matches` partidas da mesma build, `wins` delas vitórias.
fn repeated(key: &BuildKey, matches: usize, wins: usize) -> Vec<(MatchBuild, bool)> {
    (0..matches)
        .map(|index| (build(key.clone(), None), index < wins))
        .collect()
}

#[test]
fn ranks_by_observed_win_rate_with_sample_and_total() {
    let strong = unordered(&["item_a", "item_b", "item_c"]);
    let weak = unordered(&["item_d", "item_e", "item_f"]);
    let mut matches = repeated(&weak, 10, 4);
    matches.extend(repeated(&strong, 12, 9));

    let ranking = rank_builds(matches, MIN_SAMPLE, 3);

    assert_eq!(ranking.analyzed, 22);
    assert_eq!(
        ranking.builds,
        vec![
            RankedBuild {
                key: strong,
                matches: 12,
                wins: 9,
                average_times: None,
            },
            RankedBuild {
                key: weak,
                matches: 10,
                wins: 4,
                average_times: None,
            },
        ]
    );
}

#[test]
fn tie_in_win_rate_puts_the_larger_sample_first() {
    let small = unordered(&["item_a", "item_b", "item_c"]);
    let large = unordered(&["item_x", "item_y", "item_z"]);
    let mut matches = repeated(&small, 10, 5);
    matches.extend(repeated(&large, 20, 10));

    let ranking = rank_builds(matches, MIN_SAMPLE, 3);

    let keys: Vec<&BuildKey> = ranking.builds.iter().map(|ranked| &ranked.key).collect();
    assert_eq!(keys, [&large, &small]);
}

#[test]
fn build_below_minimum_sample_is_left_out() {
    let key = unordered(&["item_a", "item_b", "item_c"]);

    let ranking = rank_builds(repeated(&key, 9, 5), MIN_SAMPLE, 3);

    assert!(ranking.builds.is_empty());
    assert_eq!(ranking.analyzed, 9);
}

#[test]
fn perfect_win_rate_in_a_small_sample_does_not_outrank_enough_sample() {
    let lucky = unordered(&["item_a", "item_b", "item_c"]);
    let solid = unordered(&["item_x", "item_y", "item_z"]);
    let mut matches = repeated(&lucky, 3, 3);
    matches.extend(repeated(&solid, 10, 6));

    let ranking = rank_builds(matches, MIN_SAMPLE, 3);

    assert_eq!(ranking.builds.len(), 1);
    assert_eq!(ranking.builds[0].key, solid);
}

#[test]
fn ranking_keeps_at_most_the_limit() {
    let mut matches = Vec::new();
    for letter in ["a", "b", "c", "d"] {
        let key = unordered(&[letter, "item_y", "item_z"]);
        matches.extend(repeated(&key, 10, 5));
    }
    assert_eq!(rank_builds(matches, MIN_SAMPLE, 3).builds.len(), 3);
}

#[test]
fn ordered_build_keeps_the_average_time_of_each_item() {
    let key = ordered(&["item_black_king_bar", "item_blink", "item_satanic"]);
    let matches = (0..10)
        .map(|index| {
            let shift = if index % 2 == 0 { 60 } else { -60 };
            (
                build(key.clone(), Some(vec![900 + shift, 1200, 1800 + shift])),
                true,
            )
        })
        .collect::<Vec<_>>();

    let ranking = rank_builds(matches, MIN_SAMPLE, 3);

    assert_eq!(ranking.builds[0].average_times, Some(vec![900, 1200, 1800]));
}

fn upgrades(order: &[u32]) -> Vec<u32> {
    order.to_vec()
}

#[test]
fn most_common_ability_order_uses_the_first_ten_levels() {
    let common = [1, 2, 1, 3, 1, 4, 1, 2, 2, 2];
    let mut matches: Vec<Vec<u32>> = (0..10)
        .map(|index| {
            let mut sequence = upgrades(&common);
            // Depois do nível 10 cada partida segue diferente.
            sequence.push(100 + index);
            sequence
        })
        .collect();
    matches.extend((0..4).map(|_| upgrades(&[2, 1, 2, 3, 2, 4, 2, 1, 1, 1])));
    matches.push(upgrades(&[1, 2, 3]));

    let order = ability_order(&matches, MIN_SAMPLE).unwrap();

    assert_eq!(
        order,
        AbilityOrder {
            ability_ids: common.to_vec(),
            matches: 10,
            analyzed: 14,
        }
    );
}

#[test]
fn ability_order_without_enough_matches_is_none() {
    let matches: Vec<Vec<u32>> = (0..9)
        .map(|_| upgrades(&[1, 2, 1, 3, 1, 4, 1, 2, 2, 2]))
        .collect();
    assert_eq!(ability_order(&matches, MIN_SAMPLE), None);
    assert_eq!(ability_order(&[], MIN_SAMPLE), None);
}

#[test]
fn expensive_item_without_recipe_is_not_part_of_the_build() {
    // Blink Dagger custa 2250, mas não é montada a partir de outros itens.
    let player = analyzed(
        true,
        vec![
            buy("item_blink", 600),
            buy("item_black_king_bar", 900),
            buy("item_desolator", 1300),
            buy("item_satanic", 1800),
        ],
    );
    assert_eq!(
        build_from_match(&player, &catalog()).unwrap().key,
        ordered(&["item_black_king_bar", "item_desolator", "item_satanic"])
    );
}
