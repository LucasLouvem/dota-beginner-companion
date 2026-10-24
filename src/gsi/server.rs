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
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(bind_error) => {
            let _ = events
                .send(CoreEvent::ServerFailed(format!("{address}: {bind_error}")))
                .await;
            return;
        }
    };
    let bound_address = listener.local_addr().unwrap_or(address);
    let _ = events.send(CoreEvent::ServerListening(bound_address)).await;
    if let Err(serve_error) = axum::serve(listener, router(token, events.clone())).await {
        let _ = events
            .send(CoreEvent::ServerFailed(serve_error.to_string()))
            .await;
    }
}

async fn receive(State(state): State<ServerState>, body: Bytes) -> StatusCode {
    let received_at = SystemTime::now();
    let reject = |reason| CoreEvent::GsiRejected {
        reason,
        at: received_at,
    };

    let Ok(mut payload) = serde_json::from_slice::<Value>(&body) else {
        let _ = state.events.send(reject(RejectReason::InvalidJson)).await;
        return StatusCode::BAD_REQUEST;
    };
    let token = payload.pointer("/auth/token").and_then(Value::as_str);
    if !token_matches(token, &state.token) {
        let _ = state.events.send(reject(RejectReason::InvalidToken)).await;
        return StatusCode::UNAUTHORIZED;
    }

    let is_test = payload.get(TEST_MARKER).is_some();
    // O token não precisa ficar guardado nem aparecer na tela.
    if let Some(fields) = payload.as_object_mut() {
        fields.remove("auth");
    }
    let raw = serde_json::to_string_pretty(&payload).unwrap_or_default();
    let _ = state
        .events
        .send(CoreEvent::GsiAccepted {
            raw: raw.into(),
            at: received_at,
            is_test,
        })
        .await;
    StatusCode::OK
}

fn token_matches(received: Option<&str>, expected: &str) -> bool {
    received.is_some_and(|token| bool::from(token.as_bytes().ct_eq(expected.as_bytes())))
}

#[cfg(test)]
#[path = "../../tests/unit/gsi/server_tests.rs"]
mod tests;
