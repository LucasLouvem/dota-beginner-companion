//! Provider do OpenDota (`api.opendota.com/api`). Cada consulta vira um
//! caminho da API, e cada resposta é validada campo a campo: campo ausente
//! ou com tipo errado vira "sem dado", nunca valor inventado.

use serde_json::Value;

use crate::application::ports::{
    BoxFuture, CallBudget, CallCategory, ProviderError, StatsProvider, StatsQuery,
};
use crate::domain::patch::{Patch, PatchCatalog};

use super::http_json::HttpJsonClient;

/// Provider do OpenDota sobre o cliente HTTP de JSON.
pub struct OpenDotaProvider {
    http: HttpJsonClient,
}

impl OpenDotaProvider {
    pub fn new(http: HttpJsonClient) -> Self {
        Self { http }
    }
}

/// Caminho da API de cada consulta.
fn path(query: &StatsQuery) -> String {
    match query {
        StatsQuery::Patches => "/constants/patch".into(),
    }
}

impl StatsProvider for OpenDotaProvider {
    fn fetch<'a>(
        &'a self,
        query: &'a StatsQuery,
        category: CallCategory,
        budget: &'a dyn CallBudget,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let path = path(query);
            self.http
                .get_json(&path, category, budget)
                .await
                .map(|value| value.to_string())
        })
    }

    fn decode_patches(&self, raw: &str) -> Result<PatchCatalog, ProviderError> {
        decode_patches(raw)
    }
}

/// `/constants/patch`: lista de `{name, date, id}`, espelho do dotaconstants.
/// Registros sem `id` numérico ou sem `name` ficam de fora; sem nenhum
/// registro válido, a resposta não serve.
fn decode_patches(raw: &str) -> Result<PatchCatalog, ProviderError> {
    let path = path(&StatsQuery::Patches);
    let value: Value = parse(raw, &path)?;
    let entries = value.as_array().ok_or(ProviderError::UnexpectedFormat {
        path: path.clone(),
        detail: "a lista de patches não é uma lista",
    })?;
    let patches: Vec<Patch> = entries
        .iter()
        .filter_map(|entry| {
            Some(Patch {
                id: u32::try_from(entry.get("id")?.as_u64()?).ok()?,
                name: entry.get("name")?.as_str()?.to_owned(),
            })
        })
        .collect();
    if patches.is_empty() {
        return Err(ProviderError::UnexpectedFormat {
            path,
            detail: "nenhum patch válido na resposta",
        });
    }
    Ok(PatchCatalog::new(patches))
}

fn parse(raw: &str, path: &str) -> Result<Value, ProviderError> {
    serde_json::from_str(raw).map_err(|json_error| ProviderError::InvalidJson {
        path: path.into(),
        source: std::sync::Arc::new(json_error),
    })
}

#[cfg(test)]
#[path = "../../tests/unit/providers/opendota_tests.rs"]
mod tests;
