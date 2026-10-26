//! Imagens dos itens mostrados na página da build: cada uma é lida do cache
//! no computador ou, na primeira vez, baixada e guardada. Uma imagem que
//! falha não impede nada: o item aparece pelo nome.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use tokio::task::JoinSet;

use crate::application::error::{ToDiagnostic, log_warning};
use crate::application::ports::{ItemIconError, ItemIconSource, ItemIconStore};
use crate::application::state::RecommendationStatus;
use crate::application::use_cases::recommendations::Recommendations;
use crate::domain::game::GameSnapshot;
use crate::domain::item_catalog::ItemCatalog;

/// PNG de um item, compartilhado sem cópia entre os estados publicados.
pub type IconBytes = Arc<[u8]>;
/// Imagens já carregadas, pelo nome interno do item.
pub type ItemIcons = HashMap<String, IconBytes>;

/// Adaptadores das imagens e o catálogo com o caminho de cada uma.
#[derive(Clone)]
pub struct IconSources {
    pub source: Arc<dyn ItemIconSource>,
    pub store: Arc<dyn ItemIconStore>,
    pub items: Arc<ItemCatalog>,
}

/// Itens das builds recomendadas: inventário, mochila, neutro,
/// aprimoramento e todas as compras.
pub fn recommendation_items(recommendations: &Recommendations) -> BTreeSet<String> {
    let mut items = BTreeSet::new();
    for id in recommendations.build_ids() {
        let Some(build) = recommendations.build(&id) else {
            continue;
        };
        items.extend(build.inventory.iter().cloned());
        items.extend(build.backpack.iter().cloned());
        items.extend(build.neutral.iter().cloned());
        items.extend(build.neutral_enhancement.iter().cloned());
        items.extend(
            build
                .purchases
                .iter()
                .flatten()
                .map(|purchase| purchase.item.clone()),
        );
    }
    items
}

/// Itens que o jogador carrega no estado do GSI.
pub fn carried_items(game: &GameSnapshot) -> BTreeSet<String> {
    game.inventory
        .iter()
        .flatten()
        .map(|carried| carried.name.clone())
        .collect()
}

/// Itens das recomendações prontas; vazio nos outros estados.
pub fn status_items(status: &RecommendationStatus) -> BTreeSet<String> {
    match status {
        RecommendationStatus::Ready(recommendations) => recommendation_items(recommendations),
        _ => BTreeSet::new(),
    }
}

/// Carrega as imagens dos `items`; devolve só as que foram obtidas.
pub async fn load_icons(sources: &IconSources, items: Vec<String>) -> Vec<(String, IconBytes)> {
    let mut tasks = JoinSet::new();
    for item in items {
        let sources = sources.clone();
        tasks.spawn(async move {
            let icon = load_icon(&sources, &item).await;
            icon.map(|bytes| (item, bytes))
        });
    }
    let mut loaded = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Some(icon)) => loaded.push(icon),
            Ok(None) => {}
            // Uma tarefa interrompida só deixa o item sem imagem.
            Err(join_error) => tracing::warn!(error = %join_error, "item icon task stopped"),
        }
    }
    loaded
}

/// Imagem de um item: do cache ou da fonte, que então é guardada. Item sem
/// caminho de imagem no catálogo fica sem imagem.
pub async fn load_icon(sources: &IconSources, item: &str) -> Option<IconBytes> {
    let image_path = sources.items.by_name(item)?.image_path.clone()?;
    let store = Arc::clone(&sources.store);
    let name = item.to_owned();
    match blocking(move || store.load(&name)).await {
        Ok(Some(png)) => return Some(Arc::from(png)),
        Ok(None) => {}
        Err(cache_error) => warn(&cache_error, "could not read cached item icon"),
    }
    let png = match sources.source.fetch(&image_path).await {
        Ok(png) => png,
        Err(fetch_error) => {
            warn(&fetch_error, "could not download item icon");
            return None;
        }
    };
    let bytes: IconBytes = Arc::from(png);
    let store = Arc::clone(&sources.store);
    let (name, saved) = (item.to_owned(), Arc::clone(&bytes));
    if let Err(cache_error) = blocking(move || store.save(&name, &saved)).await {
        warn(&cache_error, "could not cache item icon");
    }
    Some(bytes)
}

fn warn(icon_error: &ItemIconError, message: &str) {
    log_warning(&icon_error.to_diagnostic(&Default::default()), message);
}

/// Roda uma operação do cache fora da thread assíncrona. Uma tarefa
/// interrompida conta como falha de leitura do cache.
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, ItemIconError> + Send + 'static,
) -> Result<T, ItemIconError> {
    tokio::task::spawn_blocking(operation)
        .await
        .unwrap_or_else(|join_error| {
            Err(ItemIconError::Cache {
                path: Default::default(),
                source: Arc::new(join_error),
            })
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/item_icons_tests.rs"]
mod tests;
