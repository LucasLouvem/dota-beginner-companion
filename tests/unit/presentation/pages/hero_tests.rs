//! Testes unitários de `src/presentation/pages/hero.rs` (regras de exibição,
//! sem widgets).

use super::*;
use crate::domain::game::{AbilityState, GamePhase};
use crate::domain::hero_catalog::AbilityInfo;

fn zeus_info() -> HeroInfo {
    HeroInfo {
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![HeroRole::Nuker, HeroRole::Carry],
        abilities: vec![
            AbilityInfo {
                name: "zuus_arc_lightning".into(),
                display_name: "Arc Lightning".into(),
            },
            AbilityInfo {
                name: "zuus_static_field".into(),
                display_name: "Static Field".into(),
            },
        ],
    }
}

fn catalog() -> HeroCatalog {
    HeroCatalog::new(vec![zeus_info()])
}

fn picked(name: &str, id: Option<u32>) -> GameSnapshot {
    GameSnapshot {
        phase: Some(GamePhase::StrategyTime),
        hero: HeroPick::Picked {
            name: name.into(),
            id,
        },
        ..GameSnapshot::menu()
    }
}

#[test]
fn picked_hero_is_found_by_internal_name() {
    let catalog = catalog();
    assert_eq!(
        picked_hero(&picked("npc_dota_hero_zuus", None), &catalog),
        Some(&zeus_info())
    );
}

#[test]
fn picked_hero_falls_back_to_id_when_name_is_unknown() {
    let catalog = catalog();
    assert_eq!(
        picked_hero(&picked("npc_dota_hero_renomeado", Some(22)), &catalog),
        Some(&zeus_info())
    );
}

#[test]
fn hero_not_in_catalog_or_not_picked_has_no_static_data() {
    let catalog = catalog();
    assert_eq!(
        picked_hero(&picked("npc_dota_hero_novo", None), &catalog),
        None
    );
    assert_eq!(picked_hero(&GameSnapshot::menu(), &catalog), None);
}

#[test]
fn abilities_from_gsi_use_display_names_and_levels() {
    let game = GameSnapshot {
        abilities: Some(vec![
            AbilityState {
                name: "zuus_arc_lightning".into(),
                level: 2,
                ultimate: false,
            },
            AbilityState {
                name: "zuus_unknown".into(),
                level: 0,
                ultimate: true,
            },
        ]),
        ..picked("npc_dota_hero_zuus", Some(22))
    };
    assert_eq!(
        ability_lines(Some(&game), Some(&zeus_info())),
        Some((
            AbilitySource::Gsi,
            vec![
                AbilityLine {
                    name: "Arc Lightning".into(),
                    level: Some(2),
                    ultimate: false,
                },
                AbilityLine {
                    name: "zuus_unknown".into(),
                    level: Some(0),
                    ultimate: true,
                },
            ]
        ))
    );
}

#[test]
fn without_gsi_abilities_the_embedded_list_has_no_levels() {
    let game = picked("npc_dota_hero_zuus", Some(22));
    assert_eq!(
        ability_lines(Some(&game), Some(&zeus_info())),
        Some((
            AbilitySource::Embedded,
            vec![
                AbilityLine {
                    name: "Arc Lightning".into(),
                    level: None,
                    ultimate: false,
                },
                AbilityLine {
                    name: "Static Field".into(),
                    level: None,
                    ultimate: false,
                },
            ]
        ))
    );
}

#[test]
fn no_abilities_without_gsi_data_and_hero() {
    assert_eq!(ability_lines(Some(&GameSnapshot::menu()), None), None);
    assert_eq!(ability_lines(None, None), None);
}

#[test]
fn events_are_described_with_display_names() {
    let catalog = catalog();
    assert_eq!(
        event_label(
            &MatchEventKind::HeroPicked {
                hero: "npc_dota_hero_zuus".into()
            },
            &catalog
        ),
        "Herói escolhido: Zeus"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::AbilityLeveled {
                ability: "zuus_arc_lightning".into(),
                level: 3
            },
            &catalog
        ),
        "Arc Lightning subiu para o nível 3"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::MatchEnded {
                result: MatchResult::Defeat
            },
            &catalog
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
            &catalog()
        ),
        "Item no inventário: item_tango"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::HeroPicked {
                hero: "npc_dota_hero_novo".into()
            },
            &catalog()
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
fn static_data_labels_are_in_portuguese() {
    assert_eq!(attribute_label(&PrimaryAttribute::Universal), "Universal");
    assert_eq!(attribute_label(&PrimaryAttribute::Strength), "Força");
    assert_eq!(attack_label(&AttackType::Melee), "Corpo a corpo");
    assert_eq!(role_label(&HeroRole::Support), "Suporte");
    assert_eq!(role_label(&HeroRole::Other("Jungler".into())), "Jungler");
}

#[test]
fn neutral_item_events_say_neutral() {
    assert_eq!(
        event_label(
            &MatchEventKind::ItemAdded {
                item: "item_weighted_dice".into(),
                placement: ItemPlacement::Neutral,
            },
            &catalog()
        ),
        "Item neutro equipado: item_weighted_dice"
    );
    assert_eq!(
        event_label(
            &MatchEventKind::ItemRemoved {
                item: "item_weighted_dice".into(),
                placement: ItemPlacement::Neutral,
            },
            &catalog()
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
            &catalog()
        ),
        "Item montado: item_satanic (item_reaver, item_claymore)"
    );
}
