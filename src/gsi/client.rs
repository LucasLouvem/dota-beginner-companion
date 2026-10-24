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
                .map_err(|e| e.to_string())?;
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

    #[tokio::test]
    async fn test_event_reaches_real_server() {
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(serve(0, "secret".into(), tx));
        let Some(CoreEvent::ServerListening(addr)) = rx.recv().await else {
            panic!("servidor não iniciou");
        };
        let tester = HttpConnectionTester::new();

        let good = GsiEndpoint {
            port: addr.port(),
            token: "secret".into(),
        };
        tester.send_test_event(&good).await.unwrap();
        assert!(matches!(
            rx.recv().await,
            Some(CoreEvent::GsiAccepted { is_test: true, .. })
        ));

        let bad = GsiEndpoint {
            token: "wrong".into(),
            ..good
        };
        assert_eq!(
            tester.send_test_event(&bad).await,
            Err("HTTP 401 Unauthorized".into())
        );
    }
}
