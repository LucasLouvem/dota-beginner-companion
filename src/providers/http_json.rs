//! Cliente HTTP de JSON para APIs externas, com tempo limite, limite de
//! tamanho da resposta, intervalo mínimo entre chamadas, nova tentativa no
//! HTTP 429 e reserva no orçamento antes de cada tentativa.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use reqwest::StatusCode;
use reqwest::header::RETRY_AFTER;
use tokio::sync::Mutex;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use crate::application::ports::{CallBudget, CallCategory, ProviderError, SharedError};

/// Identifica o aplicativo nas chamadas, como pedem as boas práticas de APIs
/// públicas.
const USER_AGENT: &str = concat!("dota-beginner-companion/", env!("CARGO_PKG_VERSION"));

/// Configuração do cliente.
#[derive(Debug, Clone)]
pub struct HttpJsonConfig {
    /// Endereço base, sem barra no fim (`https://api.opendota.com/api`).
    pub base_url: String,
    /// Tempo limite de cada tentativa, da conexão ao fim da resposta.
    pub timeout: Duration,
    /// Tamanho máximo do corpo da resposta.
    pub max_body_bytes: usize,
    /// Intervalo mínimo entre o início de duas chamadas.
    pub min_interval: Duration,
    /// Tentativas por consulta, contando a primeira.
    pub max_attempts: u32,
    /// Espera antes de nova tentativa depois de um 429 sem `Retry-After`.
    pub retry_wait: Duration,
    /// Espera máxima pedida por um `Retry-After`.
    pub max_retry_wait: Duration,
}

impl HttpJsonConfig {
    /// Valores do aplicativo para o OpenDota: 15 s por tentativa, até 8 MB por
    /// resposta, 1 s entre chamadas e até 3 tentativas.
    pub fn opendota() -> Self {
        Self {
            base_url: "https://api.opendota.com/api".into(),
            timeout: Duration::from_secs(15),
            max_body_bytes: 8 * 1024 * 1024,
            min_interval: Duration::from_secs(1),
            max_attempts: 3,
            retry_wait: Duration::from_secs(5),
            max_retry_wait: Duration::from_secs(30),
        }
    }
}

/// Falha ao preparar o cliente HTTP.
#[derive(Debug, thiserror::Error)]
pub enum HttpJsonClientError {
    #[error("não foi possível criar o cliente HTTP dos dados externos")]
    ClientBuild(#[source] reqwest::Error),
}

impl ToDiagnostic for HttpJsonClientError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("Dados externos indisponíveis"),
            "O cliente HTTP dos dados externos não pôde ser criado.",
            self,
            redactor,
        )
    }
}

/// Cliente de JSON. Uma chamada por vez: o intervalo mínimo é medido entre o
/// início de chamadas sucessivas.
pub struct HttpJsonClient {
    client: reqwest::Client,
    config: HttpJsonConfig,
    budget: Arc<dyn CallBudget>,
    /// Início da última chamada; o `Mutex` assíncrono serializa as chamadas
    /// sem bloquear a thread enquanto uma espera o intervalo.
    last_call: Mutex<Option<Instant>>,
}

impl HttpJsonClient {
    pub fn new(
        config: HttpJsonConfig,
        budget: Arc<dyn CallBudget>,
    ) -> Result<Self, HttpJsonClientError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(USER_AGENT)
            .build()
            .map_err(HttpJsonClientError::ClientBuild)?;
        Ok(Self {
            client,
            config,
            budget,
            last_call: Mutex::new(None),
        })
    }

    /// Busca `path` (como `/constants/patch`) e devolve o JSON. Cada
    /// tentativa, inclusive depois de um 429, reserva uma chamada da
    /// `category` no orçamento antes de sair.
    pub async fn get_json(
        &self,
        path: &str,
        category: CallCategory,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut attempt = 1;
        loop {
            self.budget.reserve(category, SystemTime::now())?;
            let response = self.send(path).await?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return self.read_json(path, response).await;
            }
            if attempt >= self.config.max_attempts {
                return Err(ProviderError::RateLimited { path: path.into() });
            }
            tokio::time::sleep(self.retry_wait(&response)).await;
            attempt += 1;
        }
    }

    async fn send(&self, path: &str) -> Result<reqwest::Response, ProviderError> {
        let mut last_call = self.last_call.lock().await;
        if let Some(previous) = *last_call {
            let elapsed = previous.elapsed();
            if elapsed < self.config.min_interval {
                tokio::time::sleep(self.config.min_interval - elapsed).await;
            }
        }
        *last_call = Some(Instant::now());
        self.client
            .get(format!("{}{path}", self.config.base_url))
            .send()
            .await
            .map_err(|request_error| request_failure(path, request_error))
    }

    /// `Retry-After` em segundos, limitado a `max_retry_wait`; sem ele (ou em
    /// formato de data), a espera padrão.
    fn retry_wait(&self, response: &reqwest::Response) -> Duration {
        response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map_or(self.config.retry_wait, Duration::from_secs)
            .min(self.config.max_retry_wait)
    }

    async fn read_json(
        &self,
        path: &str,
        mut response: reqwest::Response,
    ) -> Result<serde_json::Value, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::UnexpectedStatus {
                path: path.into(),
                status: status.as_u16(),
            });
        }
        let limit = self.config.max_body_bytes;
        let too_large = || ProviderError::BodyTooLarge {
            path: path.into(),
            limit,
        };
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(too_large());
        }
        // Lê aos pedaços para parar no limite mesmo sem `Content-Length`.
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|request_error| request_failure(path, request_error))?
        {
            if body.len() + chunk.len() > limit {
                return Err(too_large());
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|json_error| ProviderError::InvalidJson {
            path: path.into(),
            source: Arc::new(json_error),
        })
    }
}

/// Classifica pela categoria que o próprio reqwest informa, sem ler a mensagem.
fn request_failure(path: &str, request_error: reqwest::Error) -> ProviderError {
    let timed_out = request_error.is_timeout();
    let source: SharedError = Arc::new(request_error);
    if timed_out {
        ProviderError::TimedOut {
            path: path.into(),
            source,
        }
    } else {
        ProviderError::Unreachable {
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/providers/http_json_tests.rs"]
mod tests;
