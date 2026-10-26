//! Testes unitários de `src/presentation/pages/hero.rs` (regras de exibição,
//! sem widgets).

use super::*;
use crate::domain::game::{AbilityState, GameMode, GamePhase};
use crate::domain::game::{CarriedItem, MatchId};
use crate::domain::hero_catalog::AbilityInfo;
use crate::domain::item_catalog::ItemInfo;
use crate::domain::match_events::MatchEvent;
use crate::presentation::widgets::{RowChange, RowContent, row_changes};
use std::time::{Duration, UNIX_EPOCH};

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
        ..MatchStatus::default()
    };
    assert!(matches!(hero_view(&status), HeroView::Current(Some(_))));
}

#[test]
fn shows_last_match_when_current_state_has_no_hero() {
    let status = MatchStatus {
        game: Some(GameSnapshot::menu()),
        timeline: vec![],
        last_match: Some(summary()),
        ..MatchStatus::default()
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

fn in_progress_with_gold(gold: u32) -> MatchStatus {
    MatchStatus {
        game: Some(GameSnapshot {
            phase: Some(GamePhase::InProgress),
            hero_level: Some(17),
            stats: PlayerStats {
                gold: Some(gold),
                ..summary().stats
            },
            inventory: Some(summary().final_items),
            ..picked("npc_dota_hero_zuus", Some(22))
        }),
        timeline: vec![],
        last_match: None,
        ..MatchStatus::default()
    }
}

fn content_of(status: &MatchStatus) -> HeroContent {
    hero_content(status, None, &catalog(), &items())
}

fn row(level: Level, title: &str) -> RowContent {
    RowContent::new(level, title, "Confirmado pelo GSI")
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
fn only_the_changed_stat_row_is_updated_when_gold_changes() {
    let before = content_of(&in_progress_with_gold(100));
    let after = content_of(&in_progress_with_gold(250));

    assert_eq!(
        after.stats.last(),
        Some(&row(Level::Info, "Ouro: 250")),
        "{:?}",
        after.stats
    );
    assert_eq!(
        row_changes(&before.stats, &after.stats),
        vec![RowChange::Update(after.stats.len() - 1)]
    );
    assert_eq!(row_changes(&before.hero, &after.hero), vec![]);
    assert_eq!(row_changes(&before.items, &after.items), vec![]);
    assert_eq!(row_changes(&before.abilities, &after.abilities), vec![]);
    assert_eq!(before.title, after.title);
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
fn snapshot_without_relevant_change_produces_no_row_change() {
    let mut status = in_progress_with_gold(100);
    status.timeline = entries(0..3);
    let shown = content_of(&status);
    // Um novo `AppSnapshot` com o mesmo estado da partida (por exemplo, só o
    // diagnóstico mudou) gera o mesmo conteúdo.
    let same = content_of(&status.clone());

    assert_eq!(shown, same);
    for (shown_rows, new_rows) in [
        (&shown.hero, &same.hero),
        (&shown.stats, &same.stats),
        (&shown.items, &same.items),
        (&shown.abilities, &same.abilities),
    ] {
        assert_eq!(row_changes(shown_rows, new_rows), vec![]);
    }
    assert!(timeline_changes(&status.timeline, &status.timeline).is_empty());
}

#[test]
fn core_failure_is_the_first_hero_row() {
    let failure = CoreConnectionError::StateClosed.to_diagnostic(&Redactor::default());
    let content = hero_content(
        &in_progress_with_gold(100),
        Some(&failure),
        &catalog(),
        &items(),
    );

    assert_eq!(content.hero[0], RowContent::diagnostic(&failure));
    assert_eq!(
        content.hero[1],
        RowContent::new(Level::Ok, "Zeus", "Herói confirmado pelo GSI",)
    );
}

#[test]
fn last_match_content_has_result_rows_before_the_hero() {
    let status = MatchStatus {
        game: Some(GameSnapshot::menu()),
        timeline: vec![],
        last_match: Some(summary()),
        ..MatchStatus::default()
    };
    let content = content_of(&status);

    assert_eq!(content.title, "Última partida");
    assert_eq!(
        content.hero[..4],
        [
            row(Level::Ok, "Resultado: vitória"),
            row(Level::Ok, "Duração: 23:49"),
            row(Level::Ok, "ID da partida: 9060449998"),
            RowContent::new(Level::Ok, "Zeus", "Herói confirmado pelo GSI"),
        ]
    );
}

#[test]
fn empty_state_shows_the_placeholder_rows() {
    let content = content_of(&MatchStatus::default());

    assert_eq!(content.title, "Herói escolhido");
    assert_eq!(
        content.stats,
        vec![RowContent::new(
            Level::Info,
            "Sem estatísticas ainda",
            "Os números aparecem quando o Dota os envia, a partir da seleção de heróis.",
        )]
    );
    assert_eq!(
        content.items,
        vec![RowContent::new(
            Level::Info,
            "Sem itens para mostrar",
            "O Dota envia os itens a partir do pré-jogo.",
        )]
    );
}

fn awaiting(match_number: u64, preselected: GameMode) -> MatchStatus {
    MatchStatus {
        match_number,
        mode: ModeState::Awaiting { preselected },
        ..MatchStatus::default()
    }
}

fn confirmed(mode: GameMode, origin: ModeOrigin) -> MatchStatus {
    MatchStatus {
        match_number: 1,
        mode: ModeState::Confirmed { mode, origin },
        ..MatchStatus::default()
    }
}

#[test]
fn mode_labels_are_in_portuguese_without_repetition() {
    let labels: Vec<String> = GameMode::ALL.into_iter().map(mode_label).collect();
    assert_eq!(labels, ["Escolha Livre", "Turbo", "Contra bots"]);
    assert_eq!(mode_origin_label(ModeOrigin::Chosen), "Informado por você");
    assert_eq!(mode_origin_label(ModeOrigin::Default), "Modo padrão");
}

#[test]
fn found_match_opens_the_mode_question_once() {
    let status = awaiting(1, GameMode::Turbo);

    assert_eq!(mode_dialog_request(&status, 0), Some((1, GameMode::Turbo)));
    assert_eq!(mode_dialog_request(&status, 1), None);
}

#[test]
fn mode_question_does_not_open_without_a_waiting_match() {
    assert_eq!(mode_dialog_request(&MatchStatus::default(), 0), None);
    assert_eq!(
        mode_dialog_request(&confirmed(GameMode::Turbo, ModeOrigin::Chosen), 0),
        None
    );
}

#[test]
fn confirm_button_records_the_selected_mode_as_chosen() {
    assert_eq!(
        mode_dialog_outcome(
            ModeDialogKind::MatchFound,
            true,
            GameMode::AllPick,
            GameMode::Turbo
        ),
        Some((GameMode::Turbo, ModeOrigin::Chosen))
    );
    assert_eq!(
        mode_dialog_outcome(
            ModeDialogKind::Change,
            true,
            GameMode::Turbo,
            GameMode::Bots
        ),
        Some((GameMode::Bots, ModeOrigin::Chosen))
    );
}

#[test]
fn closing_the_match_question_keeps_the_preselected_mode_as_default() {
    assert_eq!(
        mode_dialog_outcome(
            ModeDialogKind::MatchFound,
            false,
            GameMode::AllPick,
            GameMode::Turbo
        ),
        Some((GameMode::AllPick, ModeOrigin::Default))
    );
}

#[test]
fn closing_the_change_question_changes_nothing() {
    assert_eq!(
        mode_dialog_outcome(
            ModeDialogKind::Change,
            false,
            GameMode::Turbo,
            GameMode::Bots
        ),
        None
    );
}

#[test]
fn mode_can_be_changed_only_after_it_was_informed() {
    assert_eq!(change_mode_preselection(&MatchStatus::default()), None);
    assert_eq!(
        change_mode_preselection(&awaiting(1, GameMode::AllPick)),
        None
    );
    assert_eq!(
        change_mode_preselection(&confirmed(GameMode::Bots, ModeOrigin::Default)),
        Some(GameMode::Bots)
    );
}

#[test]
fn mode_row_shows_mode_and_origin() {
    assert_eq!(
        mode_row(&ModeState::Confirmed {
            mode: GameMode::Turbo,
            origin: ModeOrigin::Chosen
        }),
        RowContent::new(Level::Info, "Modo de jogo: Turbo", "Informado por você")
    );
    assert_eq!(
        mode_row(&ModeState::Confirmed {
            mode: GameMode::AllPick,
            origin: ModeOrigin::Default
        }),
        RowContent::new(Level::Info, "Modo de jogo: Escolha Livre", "Modo padrão")
    );
}

#[test]
fn mode_row_without_informed_mode_says_so() {
    assert_eq!(
        mode_row(&ModeState::NotAsked).title,
        "Modo de jogo ainda não informado"
    );
    assert_eq!(
        mode_row(&ModeState::Awaiting {
            preselected: GameMode::AllPick
        })
        .title,
        "Modo de jogo aguardando confirmação"
    );
}

#[test]
fn cosmetic_abilities_are_hidden_and_unknown_hero_abilities_stay() {
    let game = GameSnapshot {
        abilities: Some(vec![
            AbilityState {
                name: "zuus_arc_lightning".into(),
                level: 2,
                ultimate: false,
            },
            AbilityState {
                name: "plus_high_five".into(),
                level: 1,
                ultimate: false,
            },
            AbilityState {
                name: "zuus_unknown".into(),
                level: 0,
                ultimate: false,
            },
        ]),
        ..picked("npc_dota_hero_zuus", Some(22))
    };

    let (_, lines) = ability_lines(Some(&game), Some(&zeus_info())).unwrap();

    let names: Vec<&str> = lines.iter().map(|line| line.name.as_str()).collect();
    assert_eq!(names, ["Arc Lightning", "zuus_unknown"]);
}
