//! Testes unitários de `src/providers/opendota.rs`. As respostas reais estão
//! em `tests/fixtures/opendota/`.

use super::*;
use crate::application::ports::{BudgetError, CallBudget};
use crate::providers::http_json::HttpJsonConfig;
use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const PATCH_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/opendota/constants_patch.json"
));

/// Orçamento que sempre concede.
struct UnlimitedBudget;

impl CallBudget for UnlimitedBudget {
    fn reserve(&self, _category: CallCategory, _now: SystemTime) -> Result<(), BudgetError> {
        Ok(())
    }
}

async fn provider_serving(path: &'static str, body: &'static str) -> OpenDotaProvider {
    let app = Router::new().route(path, get(move || async move { body }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = HttpJsonConfig {
        base_url: format!("http://{address}"),
        timeout: Duration::from_secs(2),
        min_interval: Duration::ZERO,
        ..HttpJsonConfig::opendota()
    };
    OpenDotaProvider::new(HttpJsonClient::new(config, Arc::new(UnlimitedBudget)).unwrap())
}

#[test]
fn real_patch_response_gives_7_41_as_current() {
    let catalog = decode_patches(PATCH_RESPONSE).unwrap();
    assert_eq!(
        catalog.current(),
        Some(&Patch {
            id: 60,
            name: "7.41".into()
        })
    );
    assert_eq!(
        catalog.by_id(59).map(|patch| patch.name.as_str()),
        Some("7.40")
    );
}

#[test]
fn patch_entries_without_numeric_id_or_name_are_left_out() {
    let catalog =
        decode_patches(r#"[{"name": "7.40", "id": 59}, {"name": "7.41", "id": "60"}, {"id": 61}]"#)
            .unwrap();
    assert_eq!(catalog.current().map(|patch| patch.id), Some(59));
}

#[test]
fn patch_response_that_is_not_a_list_has_unexpected_format() {
    let error = decode_patches(r#"{"patches": []}"#).unwrap_err();
    assert!(
        matches!(&error, ProviderError::UnexpectedFormat { path, .. } if path == "/constants/patch"),
        "{error:?}"
    );
}

#[test]
fn patch_response_without_any_valid_patch_has_unexpected_format() {
    let error = decode_patches(r#"[{"name": "7.41"}]"#).unwrap_err();
    assert!(
        matches!(error, ProviderError::UnexpectedFormat { .. }),
        "{error:?}"
    );
}

#[test]
fn patch_response_that_is_not_json_is_invalid_json() {
    let error = decode_patches("<html>").unwrap_err();
    assert!(
        matches!(error, ProviderError::InvalidJson { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn fetches_patches_from_the_constants_path() {
    let provider = provider_serving("/constants/patch", PATCH_RESPONSE).await;

    let raw = provider
        .fetch(&StatsQuery::Patches, CallCategory::OnDemand)
        .await
        .unwrap();

    assert_eq!(
        provider
            .decode_patches(&raw)
            .unwrap()
            .current()
            .map(|patch| patch.id),
        Some(60)
    );
}
