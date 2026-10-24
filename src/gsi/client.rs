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
mod tests {
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
}
