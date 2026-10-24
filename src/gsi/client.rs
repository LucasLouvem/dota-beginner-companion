//! Envia um evento de teste ao próprio servidor GSI, simulando o Dota.

use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use crate::application::ports::{BoxFuture, ConnectionTestError, ConnectionTester, SharedError};
use crate::domain::gsi_config::GsiEndpoint;

use super::server::TEST_MARKER;

/// Testador de conexão que faz um POST HTTP ao servidor GSI local.
pub struct HttpConnectionTester {
    client: reqwest::Client,
}

/// Falha ao preparar o testador de conexão.
#[derive(Debug, thiserror::Error)]
pub enum HttpConnectionTesterError {
    /// O backend TLS ou o resolvedor DNS do sistema não puderam ser iniciados.
    #[error("não foi possível criar o cliente HTTP do teste de conexão")]
    ClientBuild(#[source] reqwest::Error),
}

impl ToDiagnostic for HttpConnectionTesterError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("O aplicativo não pôde iniciar"),
            "O cliente HTTP do teste de conexão não pôde ser criado.",
            self,
            redactor,
        )
    }
}

impl HttpConnectionTester {
    /// Cria o cliente com timeout de 3 s e sem proxy.
    pub fn new() -> Result<Self, HttpConnectionTesterError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .no_proxy()
            .build()
            .map_err(HttpConnectionTesterError::ClientBuild)?;
        Ok(Self { client })
    }
}

impl ConnectionTester for HttpConnectionTester {
    fn send_test_event<'a>(
        &'a self,
        endpoint: &'a GsiEndpoint,
    ) -> BoxFuture<'a, Result<(), ConnectionTestError>> {
        Box::pin(async move {
            let body = json!({
                TEST_MARKER: true,
                "provider": { "name": "Dota Beginner Companion (teste)" },
                "auth": { "token": endpoint.token },
            });
            let response = self
                .client
                .post(endpoint.uri())
                .json(&body)
                .send()
                .await
                .map_err(classify_request_error)?;
            classify_status(response.status())
        })
    }
}

/// Classifica pela categoria que o próprio reqwest informa, sem ler a mensagem.
fn classify_request_error(request_error: reqwest::Error) -> ConnectionTestError {
    let timed_out = request_error.is_timeout();
    let connection_failed = request_error.is_connect();
    let source: SharedError = Arc::new(request_error);
    if timed_out {
        ConnectionTestError::TimedOut { source }
    } else if connection_failed {
        ConnectionTestError::Unreachable { source }
    } else {
        ConnectionTestError::Request { source }
    }
}

/// Status que o servidor GSI deste aplicativo devolve: 401 para token
/// inválido, 400 para JSON inválido e 413 para corpo grande demais.
fn classify_status(status: StatusCode) -> Result<(), ConnectionTestError> {
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => Err(ConnectionTestError::TokenRejected),
        StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE => {
            Err(ConnectionTestError::PayloadRejected)
        }
        other => Err(ConnectionTestError::UnexpectedResponse {
            details: format!("HTTP {other}"),
        }),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/gsi/client_tests.rs"]
mod tests;
