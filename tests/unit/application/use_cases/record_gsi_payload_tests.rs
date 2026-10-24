//! Testes unitários de `src/application/use_cases/record_gsi_payload.rs`.

use super::*;
use crate::application::event_bus::RejectReason;
use std::io;
use std::path::PathBuf;

fn accepted(is_test: bool) -> CoreEvent {
    CoreEvent::GsiAccepted {
        raw: "{}".into(),
        at: SystemTime::now(),
        is_test,
    }
}

fn empty_status() -> GsiLogStatus {
    GsiLogStatus {
        path: PathBuf::from("logs/gsi.jsonl"),
        written: 0,
        last_error: None,
    }
}

#[test]
fn records_events_accepted_from_dota() {
    assert!(payload_to_record(&accepted(false)).is_some());
}

#[test]
fn skips_manual_test_events() {
    assert!(payload_to_record(&accepted(true)).is_none());
}

#[test]
fn skips_rejected_requests() {
    let rejected = CoreEvent::GsiRejected {
        reason: RejectReason::InvalidToken,
        at: SystemTime::now(),
    };
    assert!(payload_to_record(&rejected).is_none());
}

#[test]
fn successful_write_increments_counter_and_clears_previous_error() {
    let mut status = GsiLogStatus {
        last_error: Some(GsiRecordError::InvalidPayload),
        ..empty_status()
    };
    apply_record_result(&mut status, Ok(()));
    assert_eq!(status.written, 1);
    assert_eq!(status.last_error, None);
}

#[test]
fn failed_write_keeps_counter_and_stores_error() {
    let mut status = empty_status();
    let write_error = GsiRecordError::Write(io::ErrorKind::PermissionDenied);
    apply_record_result(&mut status, Err(write_error));
    assert_eq!(status.written, 0);
    assert_eq!(status.last_error, Some(write_error));
}
