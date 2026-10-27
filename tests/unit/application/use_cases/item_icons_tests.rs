//! Testes unitários de `src/application/use_cases/item_icons.rs`, com a
//! fonte e o cache em memória.

use super::*;
use crate::application::ports::BoxFuture;
use crate::domain::game::CarriedItem;
use crate::domain::game::GameMode;
use crate::domain::hero_catalog::{AbilityInfo, AttackType, PrimaryAttribute};
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

fn heroes() -> Arc<HeroCatalog> {
    Arc::new(HeroCatalog::new(vec![HeroInfo {
        image_path: Some("/heroes/zuus.png".into()),
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![],
        abilities: vec![
            AbilityInfo {
                image_path: Some("/abilities/zuus_arc_lightning.png".into()),
                name: "zuus_arc_lightning".into(),
                display_name: "Arc Lightning".into(),
            },
            AbilityInfo {
                image_path: None,
                name: "zuus_no_image".into(),
                display_name: "Sem imagem".into(),
            },
        ],
    }]))
}

fn sources(source: Arc<FakeSource>, store: Arc<MemoryStore>) -> IconSources {
    IconSources {
        source,
        store,
        items: catalog(),
        heroes: heroes(),
    }
}

#[tokio::test]
async fn hero_and_ability_images_use_the_same_cache_and_download() {
    let source = Arc::new(FakeSource::default());
    let store = Arc::new(MemoryStore::default());
    let sources = sources(Arc::clone(&source), Arc::clone(&store));

    let hero = load_icon(&sources, "npc_dota_hero_zuus").await.unwrap();
    let ability = load_icon(&sources, "zuus_arc_lightning").await.unwrap();
    let again = load_icon(&sources, "npc_dota_hero_zuus").await.unwrap();

    assert_eq!(&*hero, b"png:/heroes/zuus.png");
    assert_eq!(&*ability, b"png:/abilities/zuus_arc_lightning.png");
    assert_eq!(hero, again);
    assert_eq!(
        source.fetches.load(Ordering::SeqCst),
        2,
        "o herói vem do cache na segunda vez"
    );
    assert!(
        store
            .saved
            .lock()
            .unwrap()
            .contains_key("zuus_arc_lightning")
    );
}

#[tokio::test]
async fn ability_or_hero_without_image_path_or_unknown_stays_without_image() {
    let source = Arc::new(FakeSource::default());
    let sources = sources(Arc::clone(&source), Arc::new(MemoryStore::default()));

    assert!(load_icon(&sources, "zuus_no_image").await.is_none());
    assert!(load_icon(&sources, "npc_dota_hero_unknown").await.is_none());
    assert_eq!(source.fetches.load(Ordering::SeqCst), 0);
}

#[test]
fn hero_images_are_the_portrait_and_each_ability() {
    let heroes = heroes();
    let hero = heroes.by_id(22).unwrap();
    assert_eq!(
        hero_images(hero).into_iter().collect::<Vec<_>>(),
        ["npc_dota_hero_zuus", "zuus_arc_lightning", "zuus_no_image"]
    );
}

#[test]
fn status_images_cover_the_current_hero_and_nothing_without_a_hero() {
    let heroes = heroes();
    let mut snapshot = AppSnapshot::default();
    assert!(status_hero_images(&snapshot, &heroes).is_empty());

    snapshot.match_status.game = Some(GameSnapshot {
        hero: HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: Some(22),
        },
        ..GameSnapshot::menu()
    });
    assert!(status_hero_images(&snapshot, &heroes).contains("npc_dota_hero_zuus"));
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
