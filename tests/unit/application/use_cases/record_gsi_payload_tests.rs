//! Testes unitários de `src/application/use_cases/record_gsi_payload.rs`.

use super::*;
use crate::application::event_bus::RejectReason;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

const RAW_PAYLOAD: &str = r#"{"map":{"game_state":"X"}}"#;

fn received_at() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_000)
}

fn accepted(is_test: bool) -> CoreEvent {
    CoreEvent::GsiAccepted {
        raw: RAW_PAYLOAD.into(),
        at: received_at(),
        is_test,
        game: None,
    }
}

fn empty_status() -> GsiLogStatus {
    GsiLogStatus {
        path: PathBuf::from("logs/gsi.jsonl"),
        written: 0,
        last_error: None,
        stopped: None,
    }
}

#[test]
fn records_events_accepted_from_dota_with_time_and_payload() {
    let (at, raw) = payload_to_record(&accepted(false)).expect("evento do Dota é gravado");
    assert_eq!(at, received_at());
    assert_eq!(raw.as_ref(), RAW_PAYLOAD);
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

fn write_error(kind: io::ErrorKind) -> GsiRecordError {
    GsiRecordError::Write {
        path: PathBuf::from("logs/gsi.jsonl"),
        source: Arc::new(io::Error::new(kind, "falha simulada")),
    }
}

#[test]
fn successful_write_increments_counter_and_clears_previous_error() {
    let mut status = GsiLogStatus {
        last_error: Some(write_error(io::ErrorKind::PermissionDenied)),
        ..empty_status()
    };
    apply_record_result(&mut status, Ok(()));
    assert_eq!(status.written, 1);
    assert!(status.last_error.is_none());
}

#[test]
fn failed_write_keeps_counter_and_stores_error() {
    let mut status = empty_status();
    apply_record_result(
        &mut status,
        Err(write_error(io::ErrorKind::PermissionDenied)),
    );
    assert_eq!(status.written, 0);
    let Some(GsiRecordError::Write { path, source }) = &status.last_error else {
        panic!("esperava erro de gravação: {:?}", status.last_error);
    };
    assert_eq!(path, &PathBuf::from("logs/gsi.jsonl"));
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
}
