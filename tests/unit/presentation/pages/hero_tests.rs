//! Testes unitários de `src/presentation/pages/hero.rs` (regras de exibição,
//! sem widgets).

use super::*;
use crate::domain::game::{AbilityState, GamePhase};
use crate::domain::game::{CarriedItem, MatchId};
use crate::domain::hero_catalog::AbilityInfo;
use crate::domain::item_catalog::ItemInfo;

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
        name: name.into(),
        display_name: display_name.into(),
        components: vec![],
    };
    ItemCatalog::new(vec![
        item("item_satanic", "Satanic"),
        item("item_reaver", "Reaver"),
        item("item_claymore", "Claymore"),
    ])
}

fn summary() -> MatchSummary {
    MatchSummary {
        hero: HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: Some(22),
        },
        result: MatchResult::Victory,
        duration_secs: Some(1429),
        hero_level: Some(17),
        stats: PlayerStats {
            kills: Some(12),
            deaths: Some(0),
            assists: Some(10),
            last_hits: Some(113),
            denies: Some(6),
            gold: None,
            gpm: Some(520),
            xpm: Some(697),
        },
        final_items: vec![CarriedItem {
            slot: "slot0".into(),
            name: "item_satanic".into(),
        }],
        abilities: vec![],
        match_id: MatchId::from_gsi("9060449998"),
    }
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
fn shows_current_match_when_it_has_a_picked_hero() {
    let status = MatchStatus {
        game: Some(picked("npc_dota_hero_zuus", Some(22))),
        timeline: vec![],
        last_match: Some(summary()),
    };
    assert!(matches!(hero_view(&status), HeroView::Current(Some(_))));
}

#[test]
fn shows_last_match_when_current_state_has_no_hero() {
    let status = MatchStatus {
        game: Some(GameSnapshot::menu()),
        timeline: vec![],
        last_match: Some(summary()),
    };
    assert_eq!(hero_view(&status), HeroView::LastMatch(&summary()));
}

#[test]
fn shows_empty_current_state_without_last_match() {
    let status = MatchStatus::default();
    assert_eq!(hero_view(&status), HeroView::Current(None));
}

#[test]
fn last_match_state_reuses_hero_stats_items_and_abilities() {
    let snapshot = last_match_snapshot(&summary());
    assert_eq!(snapshot.phase, Some(GamePhase::PostGame));
    assert_eq!(snapshot.hero, summary().hero);
    assert_eq!(snapshot.stats, summary().stats);
    assert_eq!(snapshot.inventory, Some(summary().final_items));
    assert_eq!(snapshot.abilities, Some(vec![]));
}

#[test]
fn result_lines_show_result_duration_and_match_id() {
    assert_eq!(
        result_lines(&summary()),
        vec![
            "Resultado: vitória".to_owned(),
            "Duração: 23:49".to_owned(),
            "ID da partida: 9060449998".to_owned(),
        ]
    );
}

#[test]
fn result_lines_skip_missing_duration_and_match_id() {
    let unknown = MatchSummary {
        result: MatchResult::Unknown,
        duration_secs: None,
        match_id: None,
        ..summary()
    };
    assert_eq!(
        result_lines(&unknown),
        vec!["Resultado não informado pelo Dota".to_owned()]
    );
}

#[test]
fn duration_uses_hours_only_when_needed() {
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(1429), "23:49");
    assert_eq!(format_duration(3723), "1:02:03");
}

#[test]
fn stat_lines_show_kda_farm_and_rates() {
    assert_eq!(
        stat_lines(&summary().stats, Some(17)),
        vec![
            "Abates / mortes / assistências: 12 / 0 / 10".to_owned(),
            "Last hits / denies: 113 / 6".to_owned(),
            "Ouro por minuto (GPM): 520".to_owned(),
            "Experiência por minuto (XPM): 697".to_owned(),
            "Nível: 17".to_owned(),
        ]
    );
}

#[test]
fn stat_lines_skip_incomplete_pairs_and_are_empty_without_data() {
    let partial = PlayerStats {
        kills: Some(3),
        last_hits: Some(10),
        ..PlayerStats::default()
    };
    assert!(stat_lines(&partial, None).is_empty());
    assert!(stat_lines(&PlayerStats::default(), None).is_empty());
}

#[test]
fn item_lines_use_display_name_and_slot_place() {
    let carried = vec![
        CarriedItem {
            slot: "slot0".into(),
            name: "item_satanic".into(),
        },
        CarriedItem {
            slot: "neutral0".into(),
            name: "item_unknown_neutral".into(),
        },
    ];
    assert_eq!(
        item_lines(&carried, &items()),
        vec![
            ("Satanic".to_owned(), "Inventário".to_owned()),
            ("item_unknown_neutral".to_owned(), "Item neutro".to_owned()),
        ]
    );
}

#[test]
fn slot_labels_cover_every_gsi_slot_kind() {
    assert_eq!(slot_label("slot7"), "Mochila");
    assert_eq!(slot_label("stash3"), "Stash");
    assert_eq!(slot_label("teleport0"), "Espaço do TP");
    assert_eq!(slot_label("slot_novo"), "slot_novo");
}
