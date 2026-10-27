//! Testes unitários de `src/providers/steam_cdn.rs`, contra um servidor HTTP
//! local em `127.0.0.1:0`, sem acessar a internet.

use super::*;
use axum::Router;
use axum::http::StatusCode as AxumStatus;
use axum::routing::get;
use std::sync::atomic::{AtomicU32, Ordering};

const ITEMS: &str = "/apps/dota2/images/dota_react/items";

fn png(size: usize) -> Vec<u8> {
    let mut bytes = PNG_SIGNATURE.to_vec();
    bytes.resize(size, 0);
    bytes
}

async fn start_server(requests: Arc<AtomicU32>) -> String {
    let counter = Arc::clone(&requests);
    let app = Router::new()
        .route(
            &format!("{ITEMS}/blink.png"),
            get(move || {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    png(100)
                }
            }),
        )
        .route(&format!("{ITEMS}/huge.png"), get(|| async { png(4096) }))
        .route(
            &format!("{ITEMS}/page.png"),
            get(|| async { "<html>não é imagem</html>" }),
        )
        .route(
            &format!("{ITEMS}/slow.png"),
            get(|| async {
                tokio::time::sleep(Duration::from_millis(600)).await;
                png(100)
            }),
        )
        .route(
            &format!("{ITEMS}/missing.png"),
            get(|| async { (AxumStatus::NOT_FOUND, "sem imagem") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

async fn icons() -> (SteamCdnIcons, Arc<AtomicU32>) {
    let requests = Arc::new(AtomicU32::new(0));
    let base_url = start_server(Arc::clone(&requests)).await;
    let icons = SteamCdnIcons::new(SteamCdnConfig {
        base_url,
        timeout: Duration::from_millis(300),
        max_bytes: 1024,
        max_concurrent: 2,
    })
    .unwrap();
    (icons, requests)
}

#[tokio::test]
async fn downloads_a_png_with_the_version_mark() {
    let (icons, requests) = icons().await;

    let bytes = icons
        .fetch(&format!("{ITEMS}/blink.png?t=1593393829403"))
        .await
        .unwrap();

    assert_eq!(bytes, png(100));
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn refuses_larger_than_the_limit_not_png_and_missing_images() {
    let (icons, _) = icons().await;

    let huge = icons.fetch(&format!("{ITEMS}/huge.png")).await;
    let page = icons.fetch(&format!("{ITEMS}/page.png")).await;
    let missing = icons.fetch(&format!("{ITEMS}/missing.png")).await;

    assert!(
        matches!(huge, Err(ItemIconError::TooLarge { limit: 1024, .. })),
        "{huge:?}"
    );
    assert!(
        matches!(page, Err(ItemIconError::NotPng { .. })),
        "{page:?}"
    );
    assert!(
        matches!(
            missing,
            Err(ItemIconError::UnexpectedStatus { status: 404, .. })
        ),
        "{missing:?}"
    );
}

#[tokio::test]
async fn slow_image_times_out() {
    let (icons, _) = icons().await;

    let slow = icons.fetch(&format!("{ITEMS}/slow.png")).await;

    assert!(
        matches!(slow, Err(ItemIconError::TimedOut { .. })),
        "{slow:?}"
    );
}

#[tokio::test]
async fn path_outside_the_image_folders_is_refused_without_a_request() {
    let (icons, requests) = icons().await;

    for path in [
        "/apps/dota2/images/dota_react/heroes/icons/zuus.png",
        "/apps/dota2/images/dota_react/items/../../secret.png",
        "https://example.com/blink.png",
    ] {
        let result = icons.fetch(path).await;
        assert!(
            matches!(result, Err(ItemIconError::UnexpectedPath { .. })),
            "{path}: {result:?}"
        );
    }
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn closed_server_is_unreachable() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let icons = SteamCdnIcons::new(SteamCdnConfig {
        base_url: format!("http://{address}"),
        ..SteamCdnConfig::default()
    })
    .unwrap();

    let result = icons.fetch(&format!("{ITEMS}/blink.png")).await;

    assert!(
        matches!(result, Err(ItemIconError::Unreachable { .. })),
        "{result:?}"
    );
}

#[test]
fn item_image_paths_are_checked_by_folder_name_and_version() {
    assert!(is_image_path(&format!("{ITEMS}/blink.png")));
    assert!(is_image_path(&format!(
        "{ITEMS}/enhancement_greedy.png?t=1"
    )));
    assert!(!is_image_path(&format!("{ITEMS}/.png")));
    assert!(!is_image_path(&format!("{ITEMS}/Blink.png")));
    assert!(!is_image_path(&format!("{ITEMS}/blink.jpg")));
    assert!(!is_image_path(&format!("{ITEMS}/blink.png?t=")));
    assert!(!is_image_path(&format!("{ITEMS}/blink.png?x=1")));
    assert!(!is_image_path(&format!("{ITEMS}/sub/blink.png")));
    assert!(is_image_path(
        "/apps/dota2/images/dota_react/heroes/zuus.png"
    ));
    assert!(is_image_path(
        "/apps/dota2/images/dota_react/abilities/zuus_arc_lightning.png"
    ));
    assert!(!is_image_path(
        "/apps/dota2/images/dota_react/heroes/icons/zuus.png"
    ));
    assert!(!is_image_path("/apps/dota2/images/other/zuus.png"));
}
