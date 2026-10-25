//! Testes unitários de `src/providers/http_json.rs`, contra um servidor HTTP
//! local em `127.0.0.1:0`, sem acessar a internet.

use super::*;
use crate::application::ports::{BudgetError, BudgetExhausted, BudgetLimit};
use axum::Router;
use axum::http::{HeaderMap, HeaderValue, StatusCode as AxumStatus};
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};

/// Orçamento em memória que concede até `limit` chamadas e conta as concedidas.
struct CountingBudget {
    limit: u32,
    granted: AtomicU32,
}

impl CountingBudget {
    fn with_limit(limit: u32) -> Arc<Self> {
        Arc::new(Self {
            limit,
            granted: AtomicU32::new(0),
        })
    }

    fn granted(&self) -> u32 {
        self.granted.load(Ordering::SeqCst)
    }
}

impl CallBudget for CountingBudget {
    fn reserve(&self, category: CallCategory, _now: SystemTime) -> Result<(), BudgetError> {
        if self.granted.load(Ordering::SeqCst) >= self.limit {
            return Err(BudgetError::Exhausted(BudgetExhausted {
                category,
                limit: BudgetLimit::DailyTotal,
            }));
        }
        self.granted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Servidor de teste. `/limited` devolve 429 até a chamada `limited_until`;
/// `requests` conta todas as chamadas recebidas.
#[derive(Clone)]
struct TestServer {
    requests: Arc<AtomicU32>,
}

async fn start_server(limited_until: u32) -> (String, TestServer) {
    let server = TestServer {
        requests: Arc::new(AtomicU32::new(0)),
    };
    let counter = Arc::clone(&server.requests);
    let limited_counter = Arc::clone(&server.requests);
    let app = Router::new()
        .route(
            "/ok",
            get(move |headers: HeaderMap| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let user_agent = headers
                        .get("user-agent")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_owned();
                    axum::Json(json!({ "patch": "7.41", "user_agent": user_agent }))
                }
            }),
        )
        .route(
            "/limited",
            get(move || {
                let counter = Arc::clone(&limited_counter);
                async move {
                    let call = counter.fetch_add(1, Ordering::SeqCst) + 1;
                    if call <= limited_until {
                        let mut headers = HeaderMap::new();
                        headers.insert("retry-after", HeaderValue::from_static("0"));
                        (AxumStatus::TOO_MANY_REQUESTS, headers, "devagar").into_response()
                    } else {
                        axum::Json(json!({ "ok": true })).into_response()
                    }
                }
            }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(600)).await;
                axum::Json(json!({ "late": true }))
            }),
        )
        .route(
            "/broken",
            get(|| async { (AxumStatus::INTERNAL_SERVER_ERROR, "falha") }),
        )
        .route("/huge", get(|| async { "x".repeat(4096) }))
        .route("/not-json", get(|| async { "<html>não é JSON</html>" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), server)
}

fn config(base_url: &str) -> HttpJsonConfig {
    HttpJsonConfig {
        base_url: base_url.into(),
        timeout: Duration::from_millis(300),
        max_body_bytes: 1024,
        min_interval: Duration::ZERO,
        max_attempts: 3,
        retry_wait: Duration::from_millis(10),
        max_retry_wait: Duration::from_millis(50),
    }
}

fn client(base_url: &str) -> HttpJsonClient {
    HttpJsonClient::new(config(base_url)).unwrap()
}

#[tokio::test]
async fn returns_json_with_one_reservation_and_the_app_user_agent() {
    let (base_url, _server) = start_server(0).await;
    let budget = CountingBudget::with_limit(10);

    let value = client(&base_url)
        .get_json("/ok", CallCategory::OnDemand, budget.as_ref())
        .await
        .unwrap();

    assert_eq!(value["patch"], "7.41");
    assert_eq!(value["user_agent"], USER_AGENT);
    assert!(USER_AGENT.starts_with("dota-beginner-companion/"));
    assert_eq!(budget.granted(), 1);
}

#[tokio::test]
async fn retries_after_429_and_each_attempt_uses_budget() {
    let (base_url, server) = start_server(1).await;
    let budget = CountingBudget::with_limit(10);

    let value = client(&base_url)
        .get_json("/limited", CallCategory::Collection, budget.as_ref())
        .await
        .unwrap();

    assert_eq!(value["ok"], true);
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    assert_eq!(budget.granted(), 2);
}

#[tokio::test]
async fn gives_up_as_rate_limited_after_the_last_attempt() {
    let (base_url, server) = start_server(u32::MAX).await;
    let budget = CountingBudget::with_limit(10);

    let result = client(&base_url)
        .get_json("/limited", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(&result, Err(ProviderError::RateLimited { path }) if path == "/limited"),
        "{result:?}"
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 3);
    assert_eq!(budget.granted(), 3);
}

#[tokio::test]
async fn exhausted_budget_sends_no_request() {
    let (base_url, server) = start_server(0).await;
    let budget = CountingBudget::with_limit(0);

    let result = client(&base_url)
        .get_json("/ok", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(
            result,
            Err(ProviderError::Budget(BudgetError::Exhausted(_)))
        ),
        "{result:?}"
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn budget_ending_during_retries_stops_the_retries() {
    let (base_url, server) = start_server(u32::MAX).await;
    let budget = CountingBudget::with_limit(2);

    let result = client(&base_url)
        .get_json("/limited", CallCategory::Collection, budget.as_ref())
        .await;

    assert!(
        matches!(result, Err(ProviderError::Budget(_))),
        "{result:?}"
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn slow_response_is_a_timeout() {
    let (base_url, _server) = start_server(0).await;
    let budget = CountingBudget::with_limit(10);

    let result = client(&base_url)
        .get_json("/slow", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(&result, Err(ProviderError::TimedOut { path, .. }) if path == "/slow"),
        "{result:?}"
    );
}

#[tokio::test]
async fn server_error_is_an_unexpected_status() {
    let (base_url, _server) = start_server(0).await;
    let budget = CountingBudget::with_limit(10);

    let result = client(&base_url)
        .get_json("/broken", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(
            result,
            Err(ProviderError::UnexpectedStatus { status: 500, .. })
        ),
        "{result:?}"
    );
}

#[tokio::test]
async fn body_over_the_limit_is_refused() {
    let (base_url, _server) = start_server(0).await;
    let budget = CountingBudget::with_limit(10);

    let result = client(&base_url)
        .get_json("/huge", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(result, Err(ProviderError::BodyTooLarge { limit: 1024, .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn body_that_is_not_json_is_invalid_json() {
    let (base_url, _server) = start_server(0).await;
    let budget = CountingBudget::with_limit(10);

    let result = client(&base_url)
        .get_json("/not-json", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(result, Err(ProviderError::InvalidJson { .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn nothing_listening_is_unreachable() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let budget = CountingBudget::with_limit(10);

    let result = client(&format!("http://{address}"))
        .get_json("/ok", CallCategory::OnDemand, budget.as_ref())
        .await;

    assert!(
        matches!(result, Err(ProviderError::Unreachable { .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn calls_keep_the_minimum_interval_between_them() {
    let (base_url, _server) = start_server(0).await;
    let budget = CountingBudget::with_limit(10);
    let spaced = HttpJsonClient::new(HttpJsonConfig {
        min_interval: Duration::from_millis(200),
        ..config(&base_url)
    })
    .unwrap();

    let started = Instant::now();
    spaced
        .get_json("/ok", CallCategory::OnDemand, budget.as_ref())
        .await
        .unwrap();
    spaced
        .get_json("/ok", CallCategory::OnDemand, budget.as_ref())
        .await
        .unwrap();

    assert!(
        started.elapsed() >= Duration::from_millis(200),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn opendota_defaults_are_the_documented_policy() {
    let opendota = HttpJsonConfig::opendota();
    assert_eq!(opendota.base_url, "https://api.opendota.com/api");
    assert_eq!(opendota.max_attempts, 3);
    assert_eq!(opendota.min_interval, Duration::from_secs(1));
}
