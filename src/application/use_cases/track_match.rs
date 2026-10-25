//! Guarda o estado de jogo e a linha do tempo da partida a partir dos
//! eventos detectados pelo `MatchTracker`.

use std::time::SystemTime;

use crate::application::state::{MatchStatus, ModeOrigin, ModeState, TimelineEntry};
use crate::domain::game::{GameMode, GamePhase, GameSnapshot};
use crate::domain::match_events::{MatchEvent, MatchEventKind, MatchSummary};

/// Quantidade máxima de eventos guardados na linha do tempo; os mais antigos
/// saem primeiro. Cobre uma partida longa sem crescer sem limite.
pub const TIMELINE_LIMIT: usize = 200;

/// Atualiza o estado atual e acrescenta os eventos. Uma partida encontrada
/// começa uma linha do tempo nova; o fim da partida guarda o resumo dela, que
/// é refeito a cada evento do pós-jogo, porque o ID da partida só chega depois
/// do primeiro deles.
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
    if game.phase == Some(GamePhase::PostGame) && status.last_match.is_some() {
        status.last_match = Some(MatchSummary::from_post_game(&game));
    }
    status.game = Some(game);
}

/// Registra a "Partida encontrada" marcada pelo usuário.
pub fn record_manual_match_found(status: &mut MatchStatus, at: SystemTime) {
    push_event(status, MatchEvent::manual_match_found(), at);
}

/// Grava o modo informado para a partida `match_number`, que passa a ser o
/// pré-selecionado na próxima. Uma resposta de partida anterior (de uma
/// pergunta aberta quando outra partida foi encontrada) é ignorada. Só os
/// campos de modo mudam: a linha do tempo e a partida continuam as mesmas.
pub fn record_mode_choice(
    status: &mut MatchStatus,
    match_number: u64,
    mode: GameMode,
    origin: ModeOrigin,
) {
    if match_number != status.match_number || status.match_number == 0 {
        return;
    }
    status.mode = ModeState::Confirmed { mode, origin };
    status.last_confirmed_mode = Some(mode);
}

/// Uma partida encontrada começa uma linha do tempo nova e abre a pergunta do
/// modo, com o último modo confirmado (ou Escolha Livre) pré-selecionado.
fn push_event(status: &mut MatchStatus, event: MatchEvent, at: SystemTime) {
    if event.kind == MatchEventKind::MatchFound {
        status.timeline.clear();
        status.last_match = None;
        status.match_number += 1;
        status.mode = ModeState::Awaiting {
            preselected: status.last_confirmed_mode.unwrap_or_default(),
        };
    }
    status.timeline.push(TimelineEntry { at, event });
    let excess = status.timeline.len().saturating_sub(TIMELINE_LIMIT);
    status.timeline.drain(..excess);
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/track_match_tests.rs"]
mod tests;
