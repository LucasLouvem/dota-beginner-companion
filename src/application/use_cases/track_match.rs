//! Guarda o estado de jogo e a linha do tempo da partida a partir dos
//! eventos detectados pelo `MatchTracker`.

use std::time::SystemTime;

use crate::application::state::{MatchStatus, TimelineEntry};
use crate::domain::game::GameSnapshot;
use crate::domain::match_events::{MatchEvent, MatchEventKind, MatchSummary};

/// Quantidade máxima de eventos guardados na linha do tempo; os mais antigos
/// saem primeiro. Cobre uma partida longa sem crescer sem limite.
pub const TIMELINE_LIMIT: usize = 200;

/// Atualiza o estado atual e acrescenta os eventos. Uma partida encontrada
/// começa uma linha do tempo nova; o fim da partida guarda o resumo dela.
pub fn record_match_update(
    status: &mut MatchStatus,
    game: GameSnapshot,
    events: Vec<MatchEvent>,
    at: SystemTime,
) {
    for event in events {
        if matches!(event.kind, MatchEventKind::MatchEnded { .. }) {
            status.last_match = Some(MatchSummary::from_post_game(&game));
        }
        push_event(status, event, at);
    }
    status.game = Some(game);
}

/// Registra a "Partida encontrada" marcada pelo usuário.
pub fn record_manual_match_found(status: &mut MatchStatus, at: SystemTime) {
    push_event(status, MatchEvent::manual_match_found(), at);
}

fn push_event(status: &mut MatchStatus, event: MatchEvent, at: SystemTime) {
    if event.kind == MatchEventKind::MatchFound {
        status.timeline.clear();
        status.last_match = None;
    }
    status.timeline.push(TimelineEntry { at, event });
    let excess = status.timeline.len().saturating_sub(TIMELINE_LIMIT);
    status.timeline.drain(..excess);
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/track_match_tests.rs"]
mod tests;
