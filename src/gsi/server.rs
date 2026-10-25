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
use tokio::sync::mpsc::error::SendError;

use crate::application::error::{Redactor, ToDiagnostic, log_error, log_warning};
use crate::application::event_bus::{CoreConnectionError, CoreEvent, GsiServerError, RejectReason};

use super::normalize;

/// Tamanho máximo aceito para o corpo de uma requisição.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Campo presente apenas nos eventos enviados pelo botão "Testar conexão".
pub const TEST_MARKER: &str = "dota_companion_test";

#[derive(Clone)]
struct ServerState {
    token: Arc<str>,
    events: mpsc::Sender<CoreEvent>,
}

impl ServerState {
    fn redactor(&self) -> Redactor {
        Redactor::new([Arc::clone(&self.token)])
    }
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
    let redactor = Redactor::new([Arc::clone(&token)]);
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(bind_error) => {
            let server_error = GsiServerError::Bind {
                address,
                source: Arc::new(bind_error),
            };
            report_server_failure(&events, server_error, &redactor).await;
            return;
        }
    };
    // `local_addr` só falha com o socket inválido, logo depois de um bind que deu certo.
    let bound_address = listener.local_addr().unwrap_or(address);
    if events
        .send(CoreEvent::ServerListening(bound_address))
        .await
        .is_err()
    {
        log_error(
            &CoreConnectionError::EventRejected.to_diagnostic(&redactor),
            "GSI server is listening but the core is not receiving events",
        );
    }
    if let Err(serve_error) = axum::serve(listener, router(token, events.clone())).await {
        let server_error = GsiServerError::Serve {
            source: Arc::new(serve_error),
        };
        report_server_failure(&events, server_error, &redactor).await;
    }
}

/// Entrega a falha ao núcleo. Se o núcleo parou, a falha vai para o log em
/// vez de se perder junto com o canal.
async fn report_server_failure(
    events: &mpsc::Sender<CoreEvent>,
    server_error: GsiServerError,
    redactor: &Redactor,
) {
    if let Err(SendError(CoreEvent::ServerFailed(server_error))) =
        events.send(CoreEvent::ServerFailed(server_error)).await
    {
        log_error(
            &server_error.to_diagnostic(redactor),
            "GSI server failed while the core was not receiving events",
        );
    }
}

async fn receive(State(state): State<ServerState>, body: Bytes) -> StatusCode {
    let received_at = SystemTime::now();

    let Ok(mut payload) = serde_json::from_slice::<Value>(&body) else {
        count_rejection(&state, RejectReason::InvalidJson, received_at).await;
        return StatusCode::BAD_REQUEST;
    };
    let token = payload.pointer("/auth/token").and_then(Value::as_str);
    if !token_matches(token, &state.token) {
        count_rejection(&state, RejectReason::InvalidToken, received_at).await;
        return StatusCode::UNAUTHORIZED;
    }

    let is_test = payload.get(TEST_MARKER).is_some();
    // O token não precisa ficar guardado nem aparecer na tela.
    if let Some(fields) = payload.as_object_mut() {
        fields.remove("auth");
    }
    let raw = serde_json::to_string_pretty(&payload)
        .expect("um Value lido de JSON tem só chaves de texto e sempre volta a ser serializado");
    // O evento de teste é sintético e não descreve uma partida.
    let game = (!is_test).then(|| Box::new(normalize::game_snapshot(&payload)));
    let delivered = state
        .events
        .send(CoreEvent::GsiAccepted {
            raw: raw.into(),
            at: received_at,
            is_test,
            game,
        })
        .await;
    if delivered.is_err() {
        // O evento devolvido pelo canal não é registrado: ele contém o payload.
        log_error(
            &CoreConnectionError::EventRejected.to_diagnostic(&state.redactor()),
            "GSI event could not be delivered to the core; answering 503",
        );
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::OK
}

/// A recusa continua valendo para quem enviou; só a contagem no Diagnóstico
/// se perde quando o núcleo parou.
async fn count_rejection(state: &ServerState, reason: RejectReason, at: SystemTime) {
    if state
        .events
        .send(CoreEvent::GsiRejected { reason, at })
        .await
        .is_err()
    {
        log_warning(
            &CoreConnectionError::EventRejected.to_diagnostic(&state.redactor()),
            "rejected GSI request could not be counted by the core",
        );
    }
}

fn token_matches(received: Option<&str>, expected: &str) -> bool {
    received.is_some_and(|token| bool::from(token.as_bytes().ct_eq(expected.as_bytes())))
}

#[cfg(test)]
#[path = "../../tests/unit/gsi/server_tests.rs"]
mod tests;
