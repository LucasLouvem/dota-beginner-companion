//! Testes unitários de `src/application/use_cases/record_gsi_event.rs`.

use super::*;
use crate::application::event_bus::{GsiServerError, RejectReason};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

fn expected_event() -> ReceivedEvent {
    ReceivedEvent {
        at: received_at(),
        raw: RAW_PAYLOAD.into(),
    }
}

#[test]
fn test_event_is_stored_apart_and_does_not_count_as_dota_event() {
    let mut status = GsiStatus::default();
    record_gsi_event(&mut status, accepted(true));

    assert_eq!(status.dota_events, 0);
    assert_eq!(status.last_dota_event, None);
    assert_eq!(status.last_test_event, Some(expected_event()));
}

#[test]
fn dota_event_increments_counter_and_becomes_last_event() {
    let mut status = GsiStatus::default();
    record_gsi_event(&mut status, accepted(false));

    assert_eq!(status.dota_events, 1);
    assert_eq!(status.last_dota_event, Some(expected_event()));
    assert_eq!(status.last_test_event, None);
}

#[test]
fn rejected_request_increments_counter_and_keeps_reason_and_time() {
    let mut status = GsiStatus::default();
    record_gsi_event(
        &mut status,
        CoreEvent::GsiRejected {
            reason: RejectReason::InvalidToken,
            at: received_at(),
        },
    );

    assert_eq!(status.rejected, 1);
    assert_eq!(
        status.last_rejection,
        Some((received_at(), RejectReason::InvalidToken))
    );
    assert_eq!(status.dota_events, 0);
}

#[test]
fn listening_server_stores_its_address() {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 53000));
    let mut status = GsiStatus::default();
    record_gsi_event(&mut status, CoreEvent::ServerListening(address));

    assert!(
        matches!(status.server, ServerState::Listening(listening) if listening == address),
        "{:?}",
        status.server
    );
}

#[test]
fn failed_server_keeps_original_error() {
    let mut status = GsiStatus::default();
    record_gsi_event(
        &mut status,
        CoreEvent::ServerFailed(GsiServerError::Serve {
            source: Arc::new(io::Error::other("conexão encerrada")),
        }),
    );

    let ServerState::Failed(GsiServerError::Serve { source }) = &status.server else {
        panic!("esperava falha do servidor: {:?}", status.server);
    };
    assert_eq!(source.to_string(), "conexão encerrada");
}
