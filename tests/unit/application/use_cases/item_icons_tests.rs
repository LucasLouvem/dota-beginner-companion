//! Testes unitários de `src/application/use_cases/item_icons.rs`, com a
//! fonte e o cache em memória.

use super::*;
use crate::application::ports::BoxFuture;
use crate::domain::game::CarriedItem;
use crate::domain::game::GameMode;
use crate::domain::item_catalog::ItemInfo;
use crate::test_support::{pro_builds, pro_win, public_win, recommendations, zeus_build};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// Fonte que devolve `png:<caminho>` ou falha para os caminhos em `failing`.
#[derive(Default)]
struct FakeSource {
    fetches: AtomicU32,
    failing: Vec<String>,
}

impl ItemIconSource for FakeSource {
    fn fetch<'a>(&'a self, image_path: &'a str) -> BoxFuture<'a, Result<Vec<u8>, ItemIconError>> {
        Box::pin(async move {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            if self.failing.iter().any(|path| path == image_path) {
                return Err(ItemIconError::UnexpectedStatus {
                    path: image_path.into(),
                    status: 404,
                });
            }
            Ok(format!("png:{image_path}").into_bytes())
        })
    }
}

/// Cache em memória; com `broken`, toda operação falha.
#[derive(Default)]
struct MemoryStore {
    saved: Mutex<HashMap<String, Vec<u8>>>,
    broken: bool,
}

impl MemoryStore {
    fn failure() -> ItemIconError {
        ItemIconError::Cache {
            path: "item-icons".into(),
            source: Arc::new(std::io::Error::other("disco cheio")),
        }
    }
}

impl ItemIconStore for MemoryStore {
    fn load(&self, item: &str) -> Result<Option<Vec<u8>>, ItemIconError> {
        if self.broken {
            return Err(Self::failure());
        }
        Ok(self.saved.lock().unwrap().get(item).cloned())
    }

    fn save(&self, item: &str, png: &[u8]) -> Result<(), ItemIconError> {
        if self.broken {
            return Err(Self::failure());
        }
        self.saved.lock().unwrap().insert(item.into(), png.to_vec());
        Ok(())
    }
}

fn catalog() -> Arc<ItemCatalog> {
    let item = |name: &str, image_path: Option<&str>| ItemInfo {
        id: 0,
        name: name.into(),
        display_name: name.into(),
        cost: None,
        components: vec![],
        image_path: image_path.map(str::to_owned),
    };
    Arc::new(ItemCatalog::new(vec![
        item("item_blink", Some("/items/blink.png")),
        item("item_tango", Some("/items/tango.png")),
        item("item_no_image", None),
    ]))
}

fn sources(source: Arc<FakeSource>, store: Arc<MemoryStore>) -> IconSources {
    IconSources {
        source,
        store,
        items: catalog(),
    }
}

#[tokio::test]
async fn first_load_downloads_and_caches_and_the_next_uses_the_cache() {
    let source = Arc::new(FakeSource::default());
    let store = Arc::new(MemoryStore::default());
    let sources = sources(Arc::clone(&source), Arc::clone(&store));

    let first = load_icon(&sources, "item_blink").await.unwrap();
    let second = load_icon(&sources, "item_blink").await.unwrap();

    assert_eq!(&*first, b"png:/items/blink.png");
    assert_eq!(first, second);
    assert_eq!(
        source.fetches.load(Ordering::SeqCst),
        1,
        "a segunda vem do cache"
    );
    assert!(store.saved.lock().unwrap().contains_key("item_blink"));
}

#[tokio::test]
async fn unknown_item_or_item_without_image_is_not_fetched() {
    let source = Arc::new(FakeSource::default());
    let sources = sources(Arc::clone(&source), Arc::default());

    assert_eq!(load_icon(&sources, "item_no_image").await, None);
    assert_eq!(load_icon(&sources, "item_unknown").await, None);
    assert_eq!(source.fetches.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_download_leaves_the_item_without_image() {
    let source = Arc::new(FakeSource {
        failing: vec!["/items/blink.png".into()],
        ..FakeSource::default()
    });
    let sources = sources(source, Arc::default());

    assert_eq!(load_icon(&sources, "item_blink").await, None);
}

#[tokio::test]
async fn broken_cache_still_shows_the_downloaded_image() {
    let store = Arc::new(MemoryStore {
        broken: true,
        ..MemoryStore::default()
    });
    let sources = sources(Arc::default(), store);

    assert!(load_icon(&sources, "item_blink").await.is_some());
}

#[tokio::test]
async fn load_icons_returns_only_the_images_obtained() {
    let source = Arc::new(FakeSource {
        failing: vec!["/items/tango.png".into()],
        ..FakeSource::default()
    });
    let sources = sources(source, Arc::default());

    let mut loaded: Vec<String> = load_icons(
        &sources,
        vec![
            "item_blink".into(),
            "item_tango".into(),
            "item_no_image".into(),
        ],
    )
    .await
    .into_iter()
    .map(|(item, _)| item)
    .collect();
    loaded.sort();

    assert_eq!(loaded, ["item_blink"]);
}

#[test]
fn recommendation_items_cover_every_build_slot_and_purchase() {
    let ready = recommendations(
        GameMode::AllPick,
        pro_builds(vec![pro_win(1, zeus_build(), None)], 1, None),
        vec![public_win(2, zeus_build())],
    );

    let items = recommendation_items(&ready);

    for item in [
        "item_blink",
        "item_black_king_bar",
        "item_smoke_of_deceit",
        "item_partisans_brand",
        "item_enhancement_greedy",
        "item_tango",
    ] {
        assert!(items.contains(item), "{item}");
    }
    assert!(status_items(&RecommendationStatus::Idle).is_empty());
}

#[test]
fn carried_items_come_from_the_gsi_inventory() {
    let game = GameSnapshot {
        inventory: Some(vec![CarriedItem {
            slot: "slot0".into(),
            name: "item_blink".into(),
        }]),
        ..GameSnapshot::menu()
    };
    assert_eq!(
        carried_items(&game),
        BTreeSet::from(["item_blink".to_owned()])
    );
    assert!(carried_items(&GameSnapshot::menu()).is_empty());
}
