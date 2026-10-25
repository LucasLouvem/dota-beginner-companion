//! Testes unitários de `src/application/use_cases/track_match.rs`.

use super::*;
use crate::domain::game::{GamePhase, HeroPick, Team};
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
