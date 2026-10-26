//! Testes unitários de `src/domain/build_progress.rs`. Os dados de exemplo
//! são escritos à mão.

use super::*;
use crate::domain::game::{AbilityState, CarriedItem};
use crate::domain::item_catalog::ItemInfo;

fn buy(item: &str, time: i32) -> Purchase {
    Purchase {
        item: item.into(),
        time,
    }
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).into()).collect()
}

fn build(inventory: &[&str], backpack: &[&str], purchases: Option<Vec<Purchase>>) -> PlayerBuild {
    PlayerBuild {
        inventory: names(inventory),
        backpack: names(backpack),
        neutral: Some("item_partisans_brand".into()),
        neutral_enhancement: None,
        purchases,
    }
}

#[test]
fn final_items_follow_the_first_purchase_and_unbought_ones_go_last() {
    let analyzed = build(
        &["item_refresher", "item_aegis", "item_blink", "item_blink"],
        &["item_magic_wand"],
        Some(vec![
            buy("item_magic_wand", -60),
            buy("item_blink", 700),
            buy("item_blink", 1500),
            buy("item_refresher", 2000),
        ]),
    );

    let order = build_item_order(&analyzed);

    assert_eq!(order.evidence, ItemOrderEvidence::PurchaseLog);
    assert_eq!(
        order.items,
        names(&[
            "item_magic_wand",
            "item_blink",
            "item_refresher",
            "item_aegis"
        ]),
        "sem repetição, o neutro fora e a Aegis (sem compra) no fim"
    );
}

#[test]
fn without_purchase_log_the_slots_give_the_order() {
    let order = build_item_order(&build(
        &["item_kaya", "item_bottle"],
        &["item_clarity"],
        None,
    ));

    assert_eq!(order.evidence, ItemOrderEvidence::InventorySlots);
    assert_eq!(
        order.items,
        names(&["item_kaya", "item_bottle", "item_clarity"])
    );
}

#[test]
fn starting_items_are_the_purchases_up_to_the_start() {
    let analyzed = build(
        &[],
        &[],
        Some(vec![
            buy("item_tango", -89),
            buy("item_branches", 0),
            buy("item_boots", 5),
        ]),
    );
    assert_eq!(
        starting_items(&analyzed),
        [buy("item_tango", -89), buy("item_branches", 0)]
    );
    assert!(starting_items(&build(&[], &[], None)).is_empty());
}

#[test]
fn purchases_are_grouped_by_window_without_losing_any() {
    let analyzed = build(
        &[],
        &[],
        Some(vec![
            buy("item_tango", -89),
            buy("item_boots", 240),
            buy("item_ward_observer", 599),
            buy("item_blink", 600),
            buy("item_bkb", 1499),
            buy("item_refresher", 1500),
        ]),
    );

    assert_eq!(
        purchases_by_window(&analyzed),
        [
            (PurchaseWindow::BeforeStart, vec![buy("item_tango", -89)]),
            (
                PurchaseWindow::Early,
                vec![buy("item_boots", 240), buy("item_ward_observer", 599)]
            ),
            (
                PurchaseWindow::Mid,
                vec![buy("item_blink", 600), buy("item_bkb", 1499)]
            ),
            (PurchaseWindow::Late, vec![buy("item_refresher", 1500)]),
        ]
    );
    assert!(purchases_by_window(&build(&[], &[], None)).is_empty());
}

fn catalog() -> ItemCatalog {
    let item = |name: &str, components: &[&str]| ItemInfo {
        id: 0,
        name: name.into(),
        display_name: name.into(),
        cost: None,
        components: names(components),
    };
    ItemCatalog::new(vec![
        item(
            "item_power_treads",
            &["item_boots", "item_gloves", "item_belt_of_strength"],
        ),
        item("item_bfury", &["item_quelling_blade", "item_demon_edge"]),
        item("item_demon_edge", &["item_claymore"]),
    ])
}

fn carrying(items: &[&str]) -> GameSnapshot {
    GameSnapshot {
        inventory: Some(
            items
                .iter()
                .enumerate()
                .map(|(slot, name)| CarriedItem {
                    slot: format!("slot{slot}"),
                    name: (*name).into(),
                })
                .collect(),
        ),
        ..GameSnapshot::menu()
    }
}

#[test]
fn held_items_include_the_components_of_assembled_items() {
    let held = held_items(&carrying(&["item_power_treads", "item_bfury"]), &catalog());

    for item in [
        "item_power_treads",
        "item_boots",
        "item_bfury",
        "item_demon_edge",
        "item_claymore",
    ] {
        assert!(held.contains(item), "{item}");
    }
    assert!(held_items(&GameSnapshot::menu(), &catalog()).is_empty());
}

#[test]
fn item_progress_marks_done_the_first_missing_as_next_and_the_rest_pending() {
    let order = names(&["item_boots", "item_power_treads", "item_blink", "item_bkb"]);
    let held = held_items(&carrying(&["item_power_treads"]), &catalog());

    let states: Vec<StepState> = item_progress(&order, &held)
        .into_iter()
        .map(|step| step.state)
        .collect();

    assert_eq!(
        states,
        [
            StepState::Done,
            StepState::Done,
            StepState::Next,
            StepState::Pending
        ]
    );
}

#[test]
fn item_progress_without_items_starts_at_the_first() {
    let steps = item_progress(&names(&["item_boots", "item_blink"]), &HashSet::new());
    assert_eq!(steps[0].state, StepState::Next);
    assert_eq!(steps[1].state, StepState::Pending);
    assert!(item_progress(&[], &HashSet::new()).is_empty());
}

fn ability(name: &str, level: u32) -> AbilityState {
    AbilityState {
        name: name.into(),
        level,
        ultimate: false,
    }
}

#[test]
fn points_spent_add_ability_levels_and_talents_without_cosmetics() {
    let game = GameSnapshot {
        abilities: Some(vec![
            ability("drow_ranger_frost_arrows", 4),
            ability("drow_ranger_multishot", 2),
            ability("plus_high_five", 1),
        ]),
        talents_learned: Some(2),
        ..GameSnapshot::menu()
    };
    assert_eq!(points_spent(&game), Some(8));
    assert_eq!(points_spent(&GameSnapshot::menu()), None);
}

#[test]
fn ability_progress_marks_spent_points_the_next_and_the_rest() {
    let upgrades = [1, 2, 1, 3];

    assert_eq!(
        ability_progress(&upgrades, Some(2)),
        [
            (1, StepState::Done),
            (2, StepState::Done),
            (1, StepState::Next),
            (3, StepState::Pending),
        ]
    );
    assert!(
        ability_progress(&upgrades, Some(9))
            .iter()
            .all(|(_, state)| *state == StepState::Done),
        "mais pontos que a build: tudo feito"
    );
    assert!(
        ability_progress(&upgrades, None)
            .iter()
            .all(|(_, state)| *state == StepState::Pending)
    );
}

#[test]
fn max_order_follows_the_last_point_of_each_ability_without_talents() {
    // 1 recebe o último ponto no índice 4, 2 no índice 6 e 3 no índice 5.
    let upgrades = [1, 2, 1, 900, 1, 3, 2, 901];

    assert_eq!(ability_max_order(&upgrades, |id| id >= 900), [1, 3, 2]);
    assert!(ability_max_order(&[], |_| false).is_empty());
}

#[test]
fn purchase_window_limits() {
    assert_eq!(PurchaseWindow::of(0), PurchaseWindow::BeforeStart);
    assert_eq!(PurchaseWindow::of(1), PurchaseWindow::Early);
    assert_eq!(PurchaseWindow::of(599), PurchaseWindow::Early);
    assert_eq!(PurchaseWindow::of(600), PurchaseWindow::Mid);
    assert_eq!(PurchaseWindow::of(1500), PurchaseWindow::Late);
}
