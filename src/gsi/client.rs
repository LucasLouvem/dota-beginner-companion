//! Envia um evento de teste ao próprio servidor GSI, simulando o Dota.

use std::time::Duration;

use serde_json::json;

use crate::application::ports::{BoxFuture, ConnectionTester};
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
    ) -> BoxFuture<'a, Result<(), String>> {
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
                .map_err(|request_error| request_error.to_string())?;
            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("HTTP {}", response.status()))
            }
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/gsi/client_tests.rs"]
mod tests;
