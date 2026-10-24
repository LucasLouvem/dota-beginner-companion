//! Testes unitários de `src/application/use_cases/record_gsi_event.rs`.

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
