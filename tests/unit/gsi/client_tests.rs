//! Testes unitários de `src/gsi/client.rs`.

use super::*;
use crate::application::event_bus::CoreEvent;
use crate::gsi::server::serve;
use tokio::sync::mpsc;

/// Inicia um servidor real em porta livre e devolve o endpoint e o canal de eventos.
async fn start_server(token: &str) -> (GsiEndpoint, mpsc::Receiver<CoreEvent>) {
    let (event_sender, mut event_receiver) = mpsc::channel(4);
    tokio::spawn(serve(0, token.into(), event_sender));
    let Some(CoreEvent::ServerListening(address)) = event_receiver.recv().await else {
        panic!("servidor não iniciou");
    };
    let endpoint = GsiEndpoint {
        port: address.port(),
        token: token.into(),
    };
    (endpoint, event_receiver)
}

#[tokio::test]
async fn test_event_with_valid_token_reaches_server_as_test() {
    let (endpoint, mut event_receiver) = start_server("secret").await;

    HttpConnectionTester::new()
        .send_test_event(&endpoint)
        .await
        .unwrap();
    assert!(matches!(
        event_receiver.recv().await,
        Some(CoreEvent::GsiAccepted { is_test: true, .. })
    ));
}

#[tokio::test]
async fn test_event_with_wrong_token_fails_with_http_status() {
    let (endpoint, _event_receiver) = start_server("secret").await;
    let wrong_token = GsiEndpoint {
        token: "wrong".into(),
        ..endpoint
    };

    assert_eq!(
        HttpConnectionTester::new()
            .send_test_event(&wrong_token)
            .await,
        Err("HTTP 401 Unauthorized".into())
    );
}
