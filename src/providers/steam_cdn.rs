//! Imagens dos itens no CDN da Steam, pelo caminho que o dotaconstants
//! informa. Só baixa caminhos da pasta de itens do Dota, com tempo limite,
//! limite de tamanho, poucas conexões ao mesmo tempo e conferência de que o
//! arquivo é PNG.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::application::ports::{BoxFuture, ItemIconError, ItemIconSource, SharedError};

use super::http_json::HttpJsonClientError;

/// Assinatura de todo arquivo PNG.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Pasta das imagens de itens no CDN; outros caminhos são recusados.
const ITEM_IMAGES_PREFIX: &str = "/apps/dota2/images/dota_react/items/";
const USER_AGENT: &str = concat!("dota-beginner-companion/", env!("CARGO_PKG_VERSION"));

/// Configuração do cliente.
#[derive(Debug, Clone)]
pub struct SteamCdnConfig {
    /// Endereço base, sem barra no fim.
    pub base_url: String,
    pub timeout: Duration,
    pub max_bytes: usize,
    /// Downloads ao mesmo tempo.
    pub max_concurrent: usize,
}

impl Default for SteamCdnConfig {
    /// Valores do aplicativo: 10 s por imagem, até 64 KB (as imagens têm
    /// cerca de 12 KB) e até 4 downloads ao mesmo tempo.
    fn default() -> Self {
        Self {
            base_url: "https://cdn.cloudflare.steamstatic.com".into(),
            timeout: Duration::from_secs(10),
            max_bytes: 64 * 1024,
            max_concurrent: 4,
        }
    }
}

/// Imagens dos itens no CDN da Steam.
pub struct SteamCdnIcons {
    client: reqwest::Client,
    config: SteamCdnConfig,
    permits: Semaphore,
}

impl SteamCdnIcons {
    pub fn new(config: SteamCdnConfig) -> Result<Self, HttpJsonClientError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(USER_AGENT)
            .build()
            .map_err(HttpJsonClientError::ClientBuild)?;
        Ok(Self {
            client,
            permits: Semaphore::new(config.max_concurrent.max(1)),
            config,
        })
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, ItemIconError> {
        if !is_item_image_path(path) {
            return Err(ItemIconError::UnexpectedPath { path: path.into() });
        }
        // O semáforo nunca é fechado, então a espera só termina com a vaga.
        let _permit = self.permits.acquire().await;
        let mut response = self
            .client
            .get(format!("{}{path}", self.config.base_url))
            .send()
            .await
            .map_err(|request_error| request_failure(path, request_error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ItemIconError::UnexpectedStatus {
                path: path.into(),
                status: status.as_u16(),
            });
        }
        let limit = self.config.max_bytes;
        let too_large = || ItemIconError::TooLarge {
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
        if !body.starts_with(&PNG_SIGNATURE) {
            return Err(ItemIconError::NotPng { path: path.into() });
        }
        Ok(body)
    }
}

impl ItemIconSource for SteamCdnIcons {
    fn fetch<'a>(&'a self, image_path: &'a str) -> BoxFuture<'a, Result<Vec<u8>, ItemIconError>> {
        Box::pin(self.download(image_path))
    }
}

/// Caminho da pasta de itens com nome simples (letras minúsculas, números,
/// `_` e `.png`), seguido ou não da marca de versão `?t=<números>`. Recusa
/// `..`, outras pastas e outros parâmetros.
pub fn is_item_image_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(ITEM_IMAGES_PREFIX) else {
        return false;
    };
    let (file, version) = match rest.split_once('?') {
        Some((file, query)) => (file, Some(query)),
        None => (rest, None),
    };
    let valid_file = file.strip_suffix(".png").is_some_and(|name| {
        !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            })
    });
    let valid_version = version.is_none_or(|query| {
        query.strip_prefix("t=").is_some_and(|digits| {
            !digits.is_empty() && digits.chars().all(|character| character.is_ascii_digit())
        })
    });
    valid_file && valid_version
}

/// Classifica pela categoria que o próprio reqwest informa, sem ler a mensagem.
fn request_failure(path: &str, request_error: reqwest::Error) -> ItemIconError {
    let timed_out = request_error.is_timeout();
    let source: SharedError = Arc::new(request_error);
    if timed_out {
        ItemIconError::TimedOut {
            path: path.into(),
            source,
        }
    } else {
        ItemIconError::Unreachable {
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/providers/steam_cdn_tests.rs"]
mod tests;
