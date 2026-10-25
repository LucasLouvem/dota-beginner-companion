//! Dados dos itens embutidos no binário, a partir do recorte do dotaconstants
//! em `assets/dotaconstants/items.json` (fonte e licença em `SOURCE.md`).

use serde::Deserialize;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use crate::domain::item_catalog::{ItemCatalog, ItemInfo};

/// Conteúdo do recorte, incluído na compilação.
const EMBEDDED_ITEMS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/dotaconstants/items.json"
));

/// O recorte embutido não pôde ser lido.
#[derive(Debug, thiserror::Error)]
#[error("os dados embutidos dos itens não são válidos")]
pub struct ItemDataError(#[source] serde_json::Error);

impl ToDiagnostic for ItemDataError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("Dados dos itens indisponíveis"),
            "Os itens aparecem com o nome interno enviado pelo GSI.",
            self,
            redactor,
        )
    }
}

#[derive(Deserialize)]
struct ItemRecord {
    name: String,
    display_name: String,
    cost: Option<u32>,
    components: Vec<String>,
}

/// Catálogo com os dados embutidos.
pub fn embedded_catalog() -> Result<ItemCatalog, ItemDataError> {
    parse_catalog(EMBEDDED_ITEMS)
}

fn parse_catalog(contents: &str) -> Result<ItemCatalog, ItemDataError> {
    let records: Vec<ItemRecord> = serde_json::from_str(contents).map_err(ItemDataError)?;
    Ok(ItemCatalog::new(
        records
            .into_iter()
            .map(|record| ItemInfo {
                name: record.name,
                display_name: record.display_name,
                cost: record.cost,
                components: record.components,
            })
            .collect(),
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/item_data_tests.rs"]
mod tests;
