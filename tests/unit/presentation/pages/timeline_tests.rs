//! Testes unitários de `src/presentation/pages/timeline.rs` (texto dos
//! eventos e comparação da linha do tempo, sem widgets).

use super::*;
use crate::domain::hero_catalog::{AbilityInfo, AttackType, HeroInfo, HeroRole, PrimaryAttribute};
use crate::domain::item_catalog::ItemInfo;
use crate::domain::match_events::MatchEvent;
use std::time::{Duration, UNIX_EPOCH};

fn zeus_info() -> HeroInfo {
    HeroInfo {
        image_path: None,
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![HeroRole::Nuker, HeroRole::Carry],
        abilities: vec![
            AbilityInfo {
                image_path: None,
                name: "zuus_arc_lightning".into(),
                display_name: "Arc Lightning".into(),
            },
            AbilityInfo {
                image_path: None,
                name: "zuus_static_field".into(),
                display_name: "Static Field".into(),
            },
        ],
    }
}

fn catalog() -> HeroCatalog {
    HeroCatalog::new(vec![zeus_info()])
}

#[test]
fn events_are_described_with_display_names() {
    let catalog = catalog();
    assert_eq!(
        event_label(
            &MatchEventKind::HeroPicked {
                hero: "npc_dota_hero_zuus".into()
            },
            &catalog,
            &ItemCatalog::default()
        ),
        "Herói escolhido: Zeus"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::AbilityLeveled {
                ability: "zuus_arc_lightning".into(),
                level: 3
            },
            &catalog,
            &ItemCatalog::default()
        ),
        "Arc Lightning subiu para o nível 3"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::MatchEnded {
                result: MatchResult::Defeat
            },
            &catalog,
            &ItemCatalog::default()
        ),
        "Partida terminou: derrota"
    );
}

#[test]
fn unknown_names_in_events_are_shown_as_received() {
    assert_eq!(
        event_label(
            &MatchEventKind::ItemAdded {
                item: "item_tango".into(),
                placement: ItemPlacement::Carried,
            },
            &catalog(),
            &ItemCatalog::default()
        ),
        "Item no inventário: item_tango"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::HeroPicked {
                hero: "npc_dota_hero_novo".into()
            },
            &catalog(),
            &ItemCatalog::default()
        ),
        "Herói escolhido: npc_dota_hero_novo"
    );
}

#[test]
fn provenance_labels_distinguish_confirmed_inferred_and_manual() {
    assert_eq!(
        provenance_label(Provenance::Confirmed),
        "Confirmado pelo GSI"
    );
    assert_eq!(
        provenance_label(Provenance::Inferred),
        "Inferido pelo aplicativo"
    );
    assert_eq!(provenance_label(Provenance::Manual), "Marcado por você");
}

#[test]
fn neutral_item_events_say_neutral() {
    assert_eq!(
        event_label(
            &MatchEventKind::ItemAdded {
                item: "item_weighted_dice".into(),
                placement: ItemPlacement::Neutral,
            },
            &catalog(),
            &ItemCatalog::default()
        ),
        "Item neutro equipado: item_weighted_dice"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::ItemRemoved {
                item: "item_weighted_dice".into(),
                placement: ItemPlacement::Neutral,
            },
            &catalog(),
            &ItemCatalog::default()
        ),
        "Item neutro removido: item_weighted_dice"
    );
}

#[test]
fn assembled_item_event_lists_components_that_left() {
    assert_eq!(
        event_label(
            &MatchEventKind::ItemAssembled {
                item: "item_satanic".into(),
                components: vec!["item_reaver".into(), "item_claymore".into()],
            },
            &catalog(),
            &ItemCatalog::default()
        ),
        "Item montado: item_satanic (item_reaver, item_claymore)"
    );
}

fn items() -> ItemCatalog {
    let item = |name: &str, display_name: &str| ItemInfo {
        id: 0,
        name: name.into(),
        display_name: display_name.into(),
        cost: None,
        image_path: None,
        components: vec![],
    };
    ItemCatalog::new(vec![
        item("item_satanic", "Satanic"),
        item("item_reaver", "Reaver"),
        item("item_claymore", "Claymore"),
    ])
}

#[test]
fn item_events_use_display_names_from_the_item_catalog() {
    assert_eq!(
        event_label(
            &MatchEventKind::ItemAssembled {
                item: "item_satanic".into(),
                components: vec!["item_reaver".into(), "item_claymore".into()],
            },
            &catalog(),
            &items()
        ),
        "Item montado: Satanic (Reaver, Claymore)"
    );
}

#[test]
fn slot_labels_cover_every_gsi_slot_kind() {
    assert_eq!(slot_label("slot7"), "Mochila");
    assert_eq!(slot_label("stash3"), "Stash");
    assert_eq!(slot_label("teleport0"), "Espaço do TP");
    assert_eq!(slot_label("slot_novo"), "slot_novo");
}

/// Evento de nível de habilidade; `level` diferencia os eventos entre si.
fn leveled_entry(level: u32) -> TimelineEntry {
    TimelineEntry {
        at: UNIX_EPOCH + Duration::from_secs(u64::from(level)),
        event: MatchEvent {
            kind: MatchEventKind::AbilityLeveled {
                ability: "zuus_arc_lightning".into(),
                level,
            },
            provenance: Provenance::Confirmed,
        },
    }
}

fn entries(levels: std::ops::Range<u32>) -> Vec<TimelineEntry> {
    levels.map(leveled_entry).collect()
}

#[test]
fn new_event_is_appended_without_rebuilding_the_timeline() {
    let shown = entries(0..10);
    let new = entries(0..11);
    assert_eq!(
        timeline_changes(&shown, &new),
        TimelineChange {
            dropped_oldest: 0,
            appended: &[leveled_entry(10)],
        }
    );
}

#[test]
fn oldest_event_leaves_when_the_timeline_exceeds_its_limit() {
    let limit = crate::application::use_cases::track_match::TIMELINE_LIMIT as u32;
    let shown = entries(0..limit);
    let new = entries(1..limit + 1);

    let change = timeline_changes(&shown, &new);

    assert_eq!(change.dropped_oldest, 1);
    assert_eq!(change.appended, &[leveled_entry(limit)]);
    assert_eq!(
        shown.len() - change.dropped_oldest + change.appended.len(),
        limit as usize
    );
}

#[test]
fn new_match_replaces_every_timeline_row() {
    let shown = entries(0..5);
    let new = vec![TimelineEntry {
        at: UNIX_EPOCH,
        event: MatchEvent::manual_match_found(),
    }];
    assert_eq!(
        timeline_changes(&shown, &new),
        TimelineChange {
            dropped_oldest: 5,
            appended: &new,
        }
    );
}

#[test]
fn empty_timeline_removes_every_shown_event() {
    let shown = entries(0..3);
    assert_eq!(
        timeline_changes(&shown, &[]),
        TimelineChange {
            dropped_oldest: 3,
            appended: &[],
        }
    );
}

#[test]
fn unchanged_timeline_has_no_change() {
    let shown = entries(0..4);
    let change = timeline_changes(&shown, &shown);
    assert!(change.is_empty(), "{change:?}");
    assert!(timeline_changes(&[], &[]).is_empty());
}

#[test]
fn second_neutral_slot_is_the_neutral_enhancement() {
    assert_eq!(slot_label("neutral0"), "Item neutro");
    assert_eq!(slot_label("neutral1"), "Aprimoramento neutro");
}
