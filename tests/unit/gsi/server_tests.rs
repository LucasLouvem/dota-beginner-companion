//! Testes unitários de `src/gsi/server.rs`.

use super::*;
use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

async fn post_to_router(body: impl Into<Body>) -> (StatusCode, Option<CoreEvent>) {
    let (event_sender, mut event_receiver) = mpsc::channel(4);
    let response = router("secret".into(), event_sender)
        .oneshot(Request::post("/").body(body.into()).unwrap())
        .await
        .unwrap();
    (response.status(), event_receiver.try_recv().ok())
}

fn is_rejection(event: &Option<CoreEvent>, expected: RejectReason) -> bool {
    matches!(event, Some(CoreEvent::GsiRejected { reason, .. }) if *reason == expected)
}

#[tokio::test]
async fn accepts_request_with_valid_token_and_hides_token_from_payload() {
    let (status, event) =
        post_to_router(r#"{"auth":{"token":"secret"},"map":{"game_state":"X"}}"#).await;
    assert_eq!(status, StatusCode::OK);
    let Some(CoreEvent::GsiAccepted { raw, is_test, .. }) = event else {
        panic!("evento inesperado: {event:?}");
    };
    assert!(!is_test);
    assert!(raw.contains("game_state"));
    assert!(!raw.contains("secret"));
}

#[tokio::test]
async fn marks_event_as_test_when_test_marker_is_present() {
    let (_, event) =
        post_to_router(r#"{"auth":{"token":"secret"},"dota_companion_test":true}"#).await;
    assert!(matches!(
        event,
        Some(CoreEvent::GsiAccepted { is_test: true, .. })
    ));
}

#[tokio::test]
async fn rejects_request_with_wrong_token() {
    let (status, event) = post_to_router(r#"{"auth":{"token":"nope"}}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(is_rejection(&event, RejectReason::InvalidToken));
}

#[tokio::test]
async fn rejects_request_without_token() {
    let (status, event) = post_to_router(r#"{"map":{}}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(is_rejection(&event, RejectReason::InvalidToken));
}

#[tokio::test]
async fn rejects_request_when_body_is_not_json() {
    let (status, event) = post_to_router("not json").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(is_rejection(&event, RejectReason::InvalidJson));
}

#[tokio::test]
async fn rejects_request_when_body_exceeds_one_megabyte() {
    let (status, event) = post_to_router(vec![b' '; MAX_BODY_BYTES + 1]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(event.is_none());
}

#[tokio::test]
async fn listens_only_on_loopback_address() {
    let (event_sender, mut event_receiver) = mpsc::channel(4);
    tokio::spawn(serve(0, "secret".into(), event_sender));
    let Some(CoreEvent::ServerListening(address)) = event_receiver.recv().await else {
        panic!("servidor não iniciou");
    };
    assert!(address.ip().is_loopback());
}
