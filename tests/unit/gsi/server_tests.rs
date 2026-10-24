//! Testes unitários de `src/gsi/server.rs`.

use super::*;
use axum::body::Body;
use axum::http::Request;
use std::time::Duration;
use tower::ServiceExt;

/// Tempo máximo de espera por um evento do servidor real.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// Envia o corpo ao router e devolve o status e todos os eventos publicados.
async fn post_to_router(body: impl Into<Body>) -> (StatusCode, Vec<CoreEvent>) {
    let (event_sender, mut event_receiver) = mpsc::channel(4);
    let response = router("secret".into(), event_sender)
        .oneshot(Request::post("/").body(body.into()).unwrap())
        .await
        .unwrap();
    let mut events = Vec::new();
    while let Ok(event) = event_receiver.try_recv() {
        events.push(event);
    }
    (response.status(), events)
}

/// Envia o corpo a um router cujo núcleo já parou (receptor descartado).
async fn post_without_core(body: impl Into<Body>) -> StatusCode {
    let (event_sender, event_receiver) = mpsc::channel(4);
    drop(event_receiver);
    router("secret".into(), event_sender)
        .oneshot(Request::post("/").body(body.into()).unwrap())
        .await
        .unwrap()
        .status()
}

/// Verifica que o único evento publicado é a recusa pelo motivo esperado.
fn assert_only_rejection(events: &[CoreEvent], expected: RejectReason) {
    assert!(
        matches!(events, [CoreEvent::GsiRejected { reason, .. }] if *reason == expected),
        "{events:?}"
    );
}

#[tokio::test]
async fn accepts_request_with_valid_token_and_hides_token_from_payload() {
    let before = SystemTime::now();
    let (status, events) =
        post_to_router(r#"{"auth":{"token":"secret"},"map":{"game_state":"X"}}"#).await;
    let after = SystemTime::now();

    assert_eq!(status, StatusCode::OK);
    let [CoreEvent::GsiAccepted { raw, at, is_test }] = events.as_slice() else {
        panic!("esperava um único evento aceito: {events:?}");
    };
    assert!(!is_test);
    assert!(before <= *at && *at <= after);
    let payload: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(payload, serde_json::json!({ "map": { "game_state": "X" } }));
}

#[tokio::test]
async fn marks_event_as_test_when_test_marker_is_present() {
    let (status, events) =
        post_to_router(r#"{"auth":{"token":"secret"},"dota_companion_test":true}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        matches!(
            events.as_slice(),
            [CoreEvent::GsiAccepted { is_test: true, .. }]
        ),
        "{events:?}"
    );
}

#[tokio::test]
async fn rejects_request_with_wrong_token() {
    let (status, events) = post_to_router(r#"{"auth":{"token":"nope"}}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_only_rejection(&events, RejectReason::InvalidToken);
}

#[tokio::test]
async fn rejects_request_without_token() {
    let (status, events) = post_to_router(r#"{"map":{}}"#).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_only_rejection(&events, RejectReason::InvalidToken);
}

#[tokio::test]
async fn rejects_request_when_body_is_not_json() {
    let (status, events) = post_to_router("not json").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_only_rejection(&events, RejectReason::InvalidJson);
}

#[tokio::test]
async fn rejects_request_when_body_exceeds_one_megabyte() {
    let (status, events) = post_to_router(vec![b' '; MAX_BODY_BYTES + 1]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(events.is_empty(), "{events:?}");
}

#[tokio::test]
async fn answers_service_unavailable_and_logs_without_payload_when_core_stopped() {
    let (_guard, logs) = crate::test_support::capture_logs();

    let status =
        post_without_core(r#"{"auth":{"token":"secret"},"player":{"name":"JogadorSecreto"}}"#)
            .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let log_text = logs.text();
    assert!(log_text.contains("ERROR"), "{log_text}");
    assert!(log_text.contains("code=CORE_UNAVAILABLE"), "{log_text}");
    assert!(!log_text.contains("secret"), "{log_text}");
    assert!(!log_text.contains("JogadorSecreto"), "{log_text}");
}

#[tokio::test]
async fn keeps_rejecting_wrong_token_and_warns_when_core_stopped() {
    let (_guard, logs) = crate::test_support::capture_logs();

    let status = post_without_core(r#"{"auth":{"token":"nope"}}"#).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let log_text = logs.text();
    assert!(log_text.contains("WARN"), "{log_text}");
    assert!(
        log_text.contains("rejected GSI request could not be counted"),
        "{log_text}"
    );
}

#[tokio::test]
async fn logs_bind_failure_when_core_cannot_receive_it() {
    let (_guard, logs) = crate::test_support::capture_logs();
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = occupied.local_addr().unwrap().port();
    let (event_sender, event_receiver) = mpsc::channel(4);
    drop(event_receiver);

    serve(port, "secret".into(), event_sender).await;

    let log_text = logs.text();
    assert!(log_text.contains("code=GSI_BIND_FAILED"), "{log_text}");
    assert!(
        log_text.contains(&format!("127.0.0.1:{port}")),
        "{log_text}"
    );
}

#[tokio::test]
async fn reports_bind_failure_with_address_and_cause_when_port_is_in_use() {
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let occupied_address = occupied.local_addr().unwrap();
    let (event_sender, mut event_receiver) = mpsc::channel(4);

    serve(occupied_address.port(), "secret".into(), event_sender).await;

    let Some(CoreEvent::ServerFailed(GsiServerError::Bind { address, source })) =
        event_receiver.try_recv().ok()
    else {
        panic!("esperava falha de bind");
    };
    assert_eq!(address, occupied_address);
    assert_eq!(source.kind(), std::io::ErrorKind::AddrInUse);
}

#[tokio::test]
async fn listens_only_on_loopback_address() {
    let (event_sender, mut event_receiver) = mpsc::channel(4);
    tokio::spawn(serve(0, "secret".into(), event_sender));
    let event = tokio::time::timeout(EVENT_TIMEOUT, event_receiver.recv())
        .await
        .expect("o servidor deve avisar que está ouvindo");
    let Some(CoreEvent::ServerListening(address)) = event else {
        panic!("servidor não iniciou: {event:?}");
    };
    assert_eq!(address.ip(), Ipv4Addr::LOCALHOST);
    assert_ne!(address.port(), 0);
}
