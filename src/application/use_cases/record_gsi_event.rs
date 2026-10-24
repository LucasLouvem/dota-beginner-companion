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
mod tests {
    use super::*;
    use crate::application::event_bus::RejectReason;
    use std::time::SystemTime;

    fn accepted(is_test: bool) -> CoreEvent {
        CoreEvent::GsiAccepted {
            raw: "{}".into(),
            at: SystemTime::now(),
            is_test,
        }
    }

    #[test]
    fn test_event_is_stored_apart_and_does_not_count_as_dota_event() {
        let mut status = GsiStatus::default();
        record_gsi_event(&mut status, accepted(true));

        assert_eq!(status.dota_events, 0);
        assert!(status.last_dota_event.is_none());
        assert!(status.last_test_event.is_some());
    }

    #[test]
    fn dota_event_increments_counter_and_becomes_last_event() {
        let mut status = GsiStatus::default();
        record_gsi_event(&mut status, accepted(false));

        assert_eq!(status.dota_events, 1);
        assert!(status.last_dota_event.is_some());
    }

    #[test]
    fn rejected_request_increments_counter_and_keeps_reason() {
        let mut status = GsiStatus::default();
        record_gsi_event(
            &mut status,
            CoreEvent::GsiRejected {
                reason: RejectReason::InvalidToken,
                at: SystemTime::now(),
            },
        );

        assert_eq!(status.rejected, 1);
        assert!(matches!(
            status.last_rejection,
            Some((_, RejectReason::InvalidToken))
        ));
    }
}
