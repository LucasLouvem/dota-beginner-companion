//! Leitura de dados externos com fallback, nesta ordem: cache ainda válido
//! (sem chamar a API), dado novo da API, cache vencido marcado como antigo,
//! dados embutidos e, sem nenhum deles, "sem dados" com a causa.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::application::ports::{CacheStore, CachedValue, Freshness, ProviderError};

/// De onde veio o valor exibido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataOrigin {
    /// Resposta da API nesta consulta.
    Live,
    /// Cache dentro do prazo de validade.
    Cached,
    /// Cache vencido: aparece marcado como antigo.
    Stale,
    /// Dados embutidos no binário.
    Embedded,
}

/// Resultado de uma leitura com fallback.
#[derive(Debug, Clone)]
pub struct Sourced<T> {
    /// `None` quando nenhuma fonte tinha o dado.
    pub value: Option<T>,
    pub origin: Option<DataOrigin>,
    /// Quando o dado foi obtido da API (agora, ou quando foi para o cache).
    pub fetched_at: Option<SystemTime>,
    /// Falha da API, quando ela foi consultada e não deu o dado.
    pub error: Option<ProviderError>,
}

/// Lê `key` com fallback. `fetch` traz o JSON da API como texto e `decode`
/// o converte; um texto que não decodifica não vai para o cache.
pub async fn load_with_fallback<T, Fetch, Future_, Decode>(
    cache: Arc<dyn CacheStore>,
    key: String,
    ttl: Duration,
    now: SystemTime,
    fetch: Fetch,
    decode: Decode,
    embedded: Option<T>,
) -> Sourced<T>
where
    Fetch: FnOnce() -> Future_,
    Future_: Future<Output = Result<String, ProviderError>>,
    Decode: Fn(&str) -> Result<T, ProviderError>,
{
    let cached = read_cache(Arc::clone(&cache), key.clone(), ttl, now).await;
    let decoded_cache = cached.and_then(|cached| {
        decode(&cached.value)
            .ok()
            .map(|value| (value, cached.freshness, cached.stored_at))
    });
    if let Some((value, Freshness::Fresh, stored_at)) = decoded_cache {
        return Sourced {
            value: Some(value),
            origin: Some(DataOrigin::Cached),
            fetched_at: Some(stored_at),
            error: None,
        };
    }
    let error = match fetch()
        .await
        .and_then(|raw| decode(&raw).map(|value| (raw, value)))
    {
        Ok((raw, value)) => {
            write_cache(cache, key, raw, now).await;
            return Sourced {
                value: Some(value),
                origin: Some(DataOrigin::Live),
                fetched_at: Some(now),
                error: None,
            };
        }
        Err(provider_error) => provider_error,
    };
    match (decoded_cache, embedded) {
        (Some((value, _, stored_at)), _) => Sourced {
            value: Some(value),
            origin: Some(DataOrigin::Stale),
            fetched_at: Some(stored_at),
            error: Some(error),
        },
        (None, Some(value)) => Sourced {
            value: Some(value),
            origin: Some(DataOrigin::Embedded),
            fetched_at: None,
            error: Some(error),
        },
        (None, None) => Sourced {
            value: None,
            origin: None,
            fetched_at: None,
            error: Some(error),
        },
    }
}

/// Falha do banco local não impede a consulta: fica registrada e a leitura
/// segue como se não houvesse cache.
async fn read_cache(
    cache: Arc<dyn CacheStore>,
    key: String,
    ttl: Duration,
    now: SystemTime,
) -> Option<CachedValue> {
    let read = tokio::task::spawn_blocking(move || cache.get(&key, ttl, now)).await;
    match read {
        Ok(Ok(cached)) => cached,
        Ok(Err(storage_error)) => {
            tracing::warn!(error = %storage_error, "could not read the external data cache");
            None
        }
        Err(join_error) => {
            tracing::warn!(error = %join_error, "external data cache read was interrupted");
            None
        }
    }
}

async fn write_cache(cache: Arc<dyn CacheStore>, key: String, raw: String, now: SystemTime) {
    let written = tokio::task::spawn_blocking(move || cache.put(&key, &raw, now)).await;
    match written {
        Ok(Ok(())) => {}
        Ok(Err(storage_error)) => {
            tracing::warn!(error = %storage_error, "could not write the external data cache");
        }
        Err(join_error) => {
            tracing::warn!(error = %join_error, "external data cache write was interrupted");
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/fallback_tests.rs"]
mod tests;
