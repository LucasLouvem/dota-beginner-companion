//! Testes unitários de `src/gsi/client.rs`.

use super::*;
use crate::application::event_bus::CoreEvent;
use crate::gsi::server::serve;
use tokio::sync::mpsc;

/// Tempo máximo de espera por um evento do servidor real.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

async fn next_event(event_receiver: &mut mpsc::Receiver<CoreEvent>) -> Option<CoreEvent> {
    tokio::time::timeout(EVENT_TIMEOUT, event_receiver.recv())
        .await
        .expect("o servidor deve publicar um evento")
}

/// Inicia um servidor real em porta livre e devolve o endpoint e o canal de eventos.
async fn start_server(token: &str) -> (GsiEndpoint, mpsc::Receiver<CoreEvent>) {
    let (event_sender, mut event_receiver) = mpsc::channel(4);
    tokio::spawn(serve(0, token.into(), event_sender));
    let Some(CoreEvent::ServerListening(address)) = next_event(&mut event_receiver).await else {
        panic!("servidor não iniciou");
    };
    let endpoint = GsiEndpoint {
        port: address.port(),
        token: token.into(),
    };
    (endpoint, event_receiver)
}

#[tokio::test]
async fn test_event_with_valid_token_reaches_server_as_test_without_token() {
    let (endpoint, mut event_receiver) = start_server("secret").await;

    HttpConnectionTester::new()
        .unwrap()
        .send_test_event(&endpoint)
        .await
        .unwrap();

    let event = next_event(&mut event_receiver).await;
    let Some(CoreEvent::GsiAccepted { raw, is_test, .. }) = event else {
        panic!("esperava evento aceito: {event:?}");
    };
    assert!(is_test);
    let payload: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        payload,
        json!({
            "dota_companion_test": true,
            "provider": { "name": "Dota Beginner Companion (teste)" },
        })
    );
}

#[tokio::test]
async fn test_event_with_wrong_token_fails_as_token_rejected() {
    let (endpoint, _event_receiver) = start_server("secret").await;
    let wrong_token = GsiEndpoint {
        token: "wrong".into(),
        ..endpoint
    };

    let result = HttpConnectionTester::new()
        .unwrap()
        .send_test_event(&wrong_token)
        .await;
    assert!(
        matches!(result, Err(ConnectionTestError::TokenRejected)),
        "{result:?}"
    );
}

#[tokio::test]
async fn test_event_fails_as_unreachable_when_nothing_listens_on_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let free_port = listener.local_addr().unwrap().port();
    drop(listener);
    let endpoint = GsiEndpoint {
        port: free_port,
        token: "secret".into(),
    };

    let result = HttpConnectionTester::new()
        .unwrap()
        .send_test_event(&endpoint)
        .await;
    assert!(
        matches!(result, Err(ConnectionTestError::Unreachable { .. })),
        "{result:?}"
    );
}

#[test]
fn classifies_server_rejections_by_status() {
    assert!(classify_status(StatusCode::OK).is_ok());
    assert!(matches!(
        classify_status(StatusCode::UNAUTHORIZED),
        Err(ConnectionTestError::TokenRejected)
    ));
    assert!(matches!(
        classify_status(StatusCode::BAD_REQUEST),
        Err(ConnectionTestError::PayloadRejected)
    ));
    assert!(matches!(
        classify_status(StatusCode::PAYLOAD_TOO_LARGE),
        Err(ConnectionTestError::PayloadRejected)
    ));
}

#[test]
fn keeps_unexpected_status_only_in_details() {
    let Err(ConnectionTestError::UnexpectedResponse { details }) =
        classify_status(StatusCode::INTERNAL_SERVER_ERROR)
    else {
        panic!("esperava resposta inesperada");
    };
    assert_eq!(details, "HTTP 500 Internal Server Error");
}
