//! Testes unitários de `src/application/use_cases/track_match.rs`.

use super::*;
use crate::application::state::{ModeOrigin, ModeState};
use crate::domain::game::{GameMode, GamePhase, HeroPick, MatchId, Team};
use crate::domain::match_events::MatchResult;
use crate::domain::match_events::Provenance;
use std::time::{Duration, UNIX_EPOCH};

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn game(phase: GamePhase) -> GameSnapshot {
    GameSnapshot {
        phase: Some(phase),
        ..GameSnapshot::menu()
    }
}

fn event(kind: MatchEventKind) -> MatchEvent {
    MatchEvent {
        kind,
        provenance: Provenance::Confirmed,
    }
}

#[test]
fn keeps_latest_game_and_appends_events_with_time() {
    let mut status = MatchStatus::default();

    record_match_update(
        &mut status,
        game(GamePhase::InProgress),
        vec![event(MatchEventKind::MatchStarted)],
        at(10),
    );
    record_match_update(
        &mut status,
        game(GamePhase::InProgress),
        vec![event(MatchEventKind::HeroDied)],
        at(20),
    );

    assert_eq!(status.game, Some(game(GamePhase::InProgress)));
    assert_eq!(
        status.timeline,
        vec![
            TimelineEntry {
                at: at(10),
                event: event(MatchEventKind::MatchStarted),
            },
            TimelineEntry {
                at: at(20),
                event: event(MatchEventKind::HeroDied),
            },
        ]
    );
}

#[test]
fn match_found_starts_a_new_timeline() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        game(GamePhase::PostGame),
        vec![event(MatchEventKind::HeroDied)],
        at(10),
    );

    record_match_update(
        &mut status,
        game(GamePhase::HeroSelection),
        vec![event(MatchEventKind::MatchFound)],
        at(30),
    );

    assert_eq!(
        status.timeline,
        vec![TimelineEntry {
            at: at(30),
            event: event(MatchEventKind::MatchFound),
        }]
    );
}

#[test]
fn manual_match_found_is_recorded_as_manual_and_keeps_game() {
    let mut status = MatchStatus {
        game: Some(game(GamePhase::StrategyTime)),
        timeline: vec![TimelineEntry {
            at: at(1),
            event: event(MatchEventKind::HeroDied),
        }],
        last_match: None,
        ..MatchStatus::default()
    };

    record_manual_match_found(&mut status, at(5));

    assert_eq!(status.game, Some(game(GamePhase::StrategyTime)));
    assert_eq!(
        status.timeline,
        vec![TimelineEntry {
            at: at(5),
            event: MatchEvent::manual_match_found(),
        }]
    );
}

#[test]
fn timeline_keeps_only_the_most_recent_events() {
    let mut status = MatchStatus::default();
    let events = (0..TIMELINE_LIMIT as u32 + 5)
        .map(|level| {
            event(MatchEventKind::AbilityLeveled {
                ability: "zuus_arc_lightning".into(),
                level,
            })
        })
        .collect();

    record_match_update(&mut status, game(GamePhase::InProgress), events, at(1));

    assert_eq!(status.timeline.len(), TIMELINE_LIMIT);
    assert_eq!(
        status.timeline[0].event.kind,
        MatchEventKind::AbilityLeveled {
            ability: "zuus_arc_lightning".into(),
            level: 5,
        }
    );
}

fn post_game_won_with(hero: &str) -> GameSnapshot {
    GameSnapshot {
        clock_time: Some(1429),
        hero: HeroPick::Picked {
            name: hero.into(),
            id: None,
        },
        team: Some(Team::Radiant),
        winner: Some(Team::Radiant),
        ..game(GamePhase::PostGame)
    }
}

fn match_ended() -> MatchEvent {
    event(MatchEventKind::MatchEnded {
        result: MatchResult::Victory,
    })
}

#[test]
fn match_end_keeps_summary_of_the_post_game_state() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(10),
    );

    let summary = status.last_match.expect("o fim da partida guarda o resumo");
    assert_eq!(summary.result, MatchResult::Victory);
    assert_eq!(summary.duration_secs, Some(1429));
    assert_eq!(
        summary.hero,
        HeroPick::Picked {
            name: "npc_dota_hero_dragon_knight".into(),
            id: None,
        }
    );
}

#[test]
fn last_match_stays_after_returning_to_menu() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(10),
    );
    record_match_update(&mut status, GameSnapshot::menu(), vec![], at(20));

    assert_eq!(status.game, Some(GameSnapshot::menu()));
    assert!(status.last_match.is_some());
}

#[test]
fn next_match_found_clears_last_match() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(10),
    );
    record_match_update(
        &mut status,
        game(GamePhase::HeroSelection),
        vec![MatchEvent {
            kind: MatchEventKind::MatchFound,
            provenance: Provenance::Inferred,
        }],
        at(20),
    );

    assert_eq!(status.last_match, None);
}

#[test]
fn manual_match_found_also_clears_last_match() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(10),
    );
    record_manual_match_found(&mut status, at(20));

    assert_eq!(status.last_match, None);
    assert_eq!(status.timeline.len(), 1);
}

#[test]
fn events_without_match_end_do_not_create_a_summary() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        game(GamePhase::InProgress),
        vec![event(MatchEventKind::HeroDied)],
        at(10),
    );
    assert_eq!(status.last_match, None);
}

#[test]
fn match_id_arriving_later_in_post_game_fills_the_summary() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(10),
    );
    assert_eq!(status.last_match.as_ref().unwrap().match_id, None);

    let with_id = GameSnapshot {
        match_id: MatchId::from_gsi("1234567890"),
        ..post_game_won_with("npc_dota_hero_dragon_knight")
    };
    record_match_update(&mut status, with_id, vec![], at(11));

    assert_eq!(
        status
            .last_match
            .as_ref()
            .and_then(|summary| summary.match_id.as_ref())
            .map(MatchId::as_str),
        Some("1234567890")
    );
}

#[test]
fn post_game_without_tracked_match_end_does_not_create_a_summary() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![],
        at(10),
    );
    assert_eq!(status.last_match, None);
}

#[test]
fn menu_after_post_game_keeps_the_summary_unchanged() {
    let mut status = MatchStatus::default();
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(10),
    );
    let summary = status.last_match.clone();
    record_match_update(&mut status, GameSnapshot::menu(), vec![], at(20));
    assert_eq!(status.last_match, summary);
}

fn match_found() -> MatchEvent {
    MatchEvent {
        kind: MatchEventKind::MatchFound,
        provenance: Provenance::Inferred,
    }
}

/// Encontra uma partida pelo GSI, como a entrada na seleção de heróis.
fn find_match(status: &mut MatchStatus, seconds: u64) {
    record_match_update(
        status,
        game(GamePhase::HeroSelection),
        vec![match_found()],
        at(seconds),
    );
}

#[test]
fn first_match_of_the_session_asks_with_all_pick_preselected() {
    let mut status = MatchStatus::default();

    find_match(&mut status, 10);

    assert_eq!(status.match_number, 1);
    assert_eq!(
        status.mode,
        ModeState::Awaiting {
            preselected: GameMode::AllPick
        }
    );
}

#[test]
fn next_match_asks_with_the_last_confirmed_mode_preselected() {
    let mut status = MatchStatus::default();
    find_match(&mut status, 10);
    record_mode_choice(&mut status, 1, GameMode::Turbo, ModeOrigin::Chosen);

    find_match(&mut status, 20);

    assert_eq!(status.match_number, 2);
    assert_eq!(
        status.mode,
        ModeState::Awaiting {
            preselected: GameMode::Turbo
        }
    );
}

#[test]
fn confirmed_choice_sets_match_mode_and_next_default() {
    let mut status = MatchStatus::default();
    find_match(&mut status, 10);

    record_mode_choice(&mut status, 1, GameMode::Bots, ModeOrigin::Chosen);

    assert_eq!(
        status.mode,
        ModeState::Confirmed {
            mode: GameMode::Bots,
            origin: ModeOrigin::Chosen
        }
    );
    assert_eq!(status.last_confirmed_mode, Some(GameMode::Bots));
}

#[test]
fn closing_without_confirming_records_default_origin_and_leaves_nothing_pending() {
    let mut status = MatchStatus::default();
    find_match(&mut status, 10);

    record_mode_choice(&mut status, 1, GameMode::AllPick, ModeOrigin::Default);

    assert_eq!(
        status.mode,
        ModeState::Confirmed {
            mode: GameMode::AllPick,
            origin: ModeOrigin::Default
        }
    );
    assert!(!matches!(status.mode, ModeState::Awaiting { .. }));
}

#[test]
fn changing_mode_later_keeps_timeline_match_and_last_match() {
    let mut status = MatchStatus::default();
    find_match(&mut status, 10);
    record_mode_choice(&mut status, 1, GameMode::AllPick, ModeOrigin::Default);
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(20),
    );
    let timeline = status.timeline.clone();
    let last_match = status.last_match.clone();

    record_mode_choice(&mut status, 1, GameMode::Turbo, ModeOrigin::Chosen);

    assert_eq!(status.timeline, timeline);
    assert_eq!(status.last_match, last_match);
    assert_eq!(status.match_number, 1);
    assert_eq!(
        status.mode,
        ModeState::Confirmed {
            mode: GameMode::Turbo,
            origin: ModeOrigin::Chosen
        }
    );
}

#[test]
fn answer_for_an_earlier_match_is_ignored() {
    let mut status = MatchStatus::default();
    find_match(&mut status, 10);
    find_match(&mut status, 20);

    record_mode_choice(&mut status, 1, GameMode::Turbo, ModeOrigin::Default);

    assert_eq!(
        status.mode,
        ModeState::Awaiting {
            preselected: GameMode::AllPick
        }
    );
    assert_eq!(status.last_confirmed_mode, None);
}

#[test]
fn choice_before_any_match_is_ignored() {
    let mut status = MatchStatus::default();

    record_mode_choice(&mut status, 0, GameMode::Turbo, ModeOrigin::Chosen);

    assert_eq!(status.mode, ModeState::NotAsked);
    assert_eq!(status.last_confirmed_mode, None);
}

#[test]
fn confirmed_mode_stays_with_the_last_match_after_returning_to_menu() {
    let mut status = MatchStatus::default();
    find_match(&mut status, 10);
    record_mode_choice(&mut status, 1, GameMode::Turbo, ModeOrigin::Chosen);
    record_match_update(
        &mut status,
        post_game_won_with("npc_dota_hero_dragon_knight"),
        vec![match_ended()],
        at(20),
    );

    record_match_update(&mut status, GameSnapshot::menu(), vec![], at(30));

    assert!(status.last_match.is_some());
    assert_eq!(
        status.mode,
        ModeState::Confirmed {
            mode: GameMode::Turbo,
            origin: ModeOrigin::Chosen
        }
    );
}

#[test]
fn new_session_state_forgets_the_last_confirmed_mode() {
    let mut previous_session = MatchStatus::default();
    find_match(&mut previous_session, 10);
    record_mode_choice(
        &mut previous_session,
        1,
        GameMode::Turbo,
        ModeOrigin::Chosen,
    );

    let mut new_session = MatchStatus::default();
    find_match(&mut new_session, 20);

    assert_eq!(
        new_session.mode,
        ModeState::Awaiting {
            preselected: GameMode::AllPick
        }
    );
}

#[test]
fn match_without_id_does_not_preselect_bots() {
    let mut status = MatchStatus::default();
    let without_id = GameSnapshot {
        match_id: None,
        ..game(GamePhase::HeroSelection)
    };

    record_match_update(&mut status, without_id, vec![match_found()], at(10));

    assert_eq!(
        status.mode,
        ModeState::Awaiting {
            preselected: GameMode::AllPick
        }
    );
}

#[test]
fn manual_match_found_also_asks_for_the_mode() {
    let mut status = MatchStatus::default();

    record_manual_match_found(&mut status, at(10));

    assert_eq!(status.match_number, 1);
    assert_eq!(
        status.mode,
        ModeState::Awaiting {
            preselected: GameMode::AllPick
        }
    );
}
