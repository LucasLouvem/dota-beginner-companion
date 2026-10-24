//! Testes unitários de `src/application/use_cases/track_match.rs`.

use super::*;
use crate::domain::game::GamePhase;
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
