//! Coletor GSI: servidor HTTP que só escuta em 127.0.0.1 e só aceita
//! requisições com o token configurado.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::routing::post;
use serde_json::Value;
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::application::event_bus::{CoreEvent, RejectReason};

/// Tamanho máximo aceito para o corpo de uma requisição.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Campo presente apenas nos eventos enviados pelo botão "Testar conexão".
pub const TEST_MARKER: &str = "dota_companion_test";

#[derive(Clone)]
struct ServerState {
    token: Arc<str>,
    events: mpsc::Sender<CoreEvent>,
}

/// Rotas do servidor GSI com validação de token e limite de corpo.
pub fn router(token: Arc<str>, events: mpsc::Sender<CoreEvent>) -> Router {
    Router::new()
        .route("/", post(receive))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(ServerState { token, events })
}

/// Escuta em `127.0.0.1:<port>` até o processo terminar. Falhas de bind
/// viram `CoreEvent::ServerFailed` para aparecerem no Diagnóstico.
pub async fn serve(port: u16, token: Arc<str>, events: mpsc::Sender<CoreEvent>) {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            let _ = events
                .send(CoreEvent::ServerFailed(format!("{addr}: {e}")))
                .await;
            return;
        }
    };
    let local = listener.local_addr().unwrap_or(addr);
    let _ = events.send(CoreEvent::ServerListening(local)).await;
    if let Err(e) = axum::serve(listener, router(token, events.clone())).await {
        let _ = events.send(CoreEvent::ServerFailed(e.to_string())).await;
    }
}

async fn receive(State(state): State<ServerState>, body: Bytes) -> StatusCode {
    let at = SystemTime::now();
    let reject = |reason| CoreEvent::GsiRejected { reason, at };

    let Ok(mut value) = serde_json::from_slice::<Value>(&body) else {
        let _ = state.events.send(reject(RejectReason::InvalidJson)).await;
        return StatusCode::BAD_REQUEST;
    };
    let token = value.pointer("/auth/token").and_then(Value::as_str);
    if !token_matches(token, &state.token) {
        let _ = state.events.send(reject(RejectReason::InvalidToken)).await;
        return StatusCode::UNAUTHORIZED;
    }

    let is_test = value.get(TEST_MARKER).is_some();
    // O token não precisa ficar guardado nem aparecer na tela.
    if let Some(obj) = value.as_object_mut() {
        obj.remove("auth");
    }
    let raw = serde_json::to_string_pretty(&value).unwrap_or_default();
    let _ = state
        .events
        .send(CoreEvent::GsiAccepted {
            raw: raw.into(),
            at,
            is_test,
        })
        .await;
    StatusCode::OK
}

fn token_matches(received: Option<&str>, expected: &str) -> bool {
    received.is_some_and(|r| bool::from(r.as_bytes().ct_eq(expected.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn post_body(body: impl Into<Body>) -> (StatusCode, Option<CoreEvent>) {
        let (tx, mut rx) = mpsc::channel(4);
        let response = router("secret".into(), tx)
            .oneshot(Request::post("/").body(body.into()).unwrap())
            .await
            .unwrap();
        (response.status(), rx.try_recv().ok())
    }

    #[tokio::test]
    async fn accepts_valid_token_and_strips_auth() {
        let (status, event) =
            post_body(r#"{"auth":{"token":"secret"},"map":{"game_state":"X"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        let Some(CoreEvent::GsiAccepted { raw, is_test, .. }) = event else {
            panic!("evento inesperado: {event:?}");
        };
        assert!(!is_test);
        assert!(raw.contains("game_state"));
        assert!(!raw.contains("secret"));
    }

    #[tokio::test]
    async fn flags_test_events() {
        let (_, event) =
            post_body(r#"{"auth":{"token":"secret"},"dota_companion_test":true}"#).await;
        assert!(matches!(
            event,
            Some(CoreEvent::GsiAccepted { is_test: true, .. })
        ));
    }

    #[tokio::test]
    async fn rejects_wrong_or_missing_token() {
        for body in [r#"{"auth":{"token":"nope"}}"#, r#"{"map":{}}"#] {
            let (status, event) = post_body(body).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert!(matches!(
                event,
                Some(CoreEvent::GsiRejected {
                    reason: RejectReason::InvalidToken,
                    ..
                })
            ));
        }
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        let (status, event) = post_body("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(matches!(
            event,
            Some(CoreEvent::GsiRejected {
                reason: RejectReason::InvalidJson,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn rejects_oversized_body() {
        let (status, event) = post_body(vec![b' '; MAX_BODY_BYTES + 1]).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(event.is_none());
    }

    #[tokio::test]
    async fn serve_binds_only_to_loopback() {
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(serve(0, "secret".into(), tx));
        let Some(CoreEvent::ServerListening(addr)) = rx.recv().await else {
            panic!("servidor não iniciou");
        };
        assert!(addr.ip().is_loopback());
    }
}
