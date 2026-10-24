//! Aplica eventos do coletor GSI ao status de conexão.

use crate::application::event_bus::CoreEvent;
use crate::application::state::{GsiStatus, ReceivedEvent, ServerState};

/// Atualiza o status de conexão a partir de um evento do coletor GSI.
pub fn record_gsi_event(status: &mut GsiStatus, event: CoreEvent) {
    match event {
        CoreEvent::ServerListening(address) => status.server = ServerState::Listening(address),
        CoreEvent::ServerFailed(error) => status.server = ServerState::Failed(error),
        CoreEvent::GsiAccepted { raw, at, is_test } => {
            let received = Some(ReceivedEvent { at, raw });
            if is_test {
                status.last_test_event = received;
            } else {
                status.dota_events += 1;
                status.last_dota_event = received;
            }
        }
        CoreEvent::GsiRejected { reason, at } => {
            status.rejected += 1;
            status.last_rejection = Some((at, reason));
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/record_gsi_event_tests.rs"]
mod tests;
