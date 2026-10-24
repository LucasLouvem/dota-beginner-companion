//! Aplica eventos do coletor GSI ao status de conexão.

use crate::application::event_bus::CoreEvent;
use crate::application::state::{GsiStatus, ReceivedEvent, ServerState};

/// Atualiza o status de conexão a partir de um evento do coletor GSI.
pub fn record_gsi_event(status: &mut GsiStatus, event: CoreEvent) {
    match event {
        CoreEvent::ServerListening(addr) => status.server = ServerState::Listening(addr),
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
mod tests {
    use super::*;
    use crate::application::event_bus::RejectReason;
    use std::time::SystemTime;

    #[test]
    fn test_events_do_not_count_as_dota_events() {
        let mut status = GsiStatus::default();
        let at = SystemTime::now();
        record_gsi_event(
            &mut status,
            CoreEvent::GsiAccepted {
                raw: "{}".into(),
                at,
                is_test: true,
            },
        );
        assert_eq!(status.dota_events, 0);
        assert!(status.last_dota_event.is_none());
        assert!(status.last_test_event.is_some());

        record_gsi_event(
            &mut status,
            CoreEvent::GsiAccepted {
                raw: "{}".into(),
                at,
                is_test: false,
            },
        );
        record_gsi_event(
            &mut status,
            CoreEvent::GsiRejected {
                reason: RejectReason::InvalidToken,
                at,
            },
        );
        assert_eq!(status.dota_events, 1);
        assert_eq!(status.rejected, 1);
    }
}
