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

impl HttpConnectionTester {
    /// Cria o cliente com timeout de 3 s e sem proxy.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .no_proxy()
            .build()
            .expect("cliente HTTP local");
        Self { client }
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
