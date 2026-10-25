//! IDs das habilidades embutidos no binário, a partir do recorte do
//! dotaconstants em `assets/dotaconstants/ability_ids.json` (fonte e licença
//! em `SOURCE.md`).

use std::collections::HashMap;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use crate::domain::ability_catalog::AbilityIdCatalog;

/// Conteúdo do recorte, incluído na compilação.
const EMBEDDED_ABILITY_IDS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/dotaconstants/ability_ids.json"
));

/// O recorte embutido não pôde ser lido.
#[derive(Debug, thiserror::Error)]
pub enum AbilityIdDataError {
    #[error("os dados embutidos dos IDs das habilidades não são JSON válido")]
    Json(#[source] serde_json::Error),
    #[error("o ID de habilidade {0:?} dos dados embutidos não é um número")]
    InvalidId(String),
}

impl ToDiagnostic for AbilityIdDataError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("Dados das habilidades indisponíveis"),
            "A ordem das habilidades aparece sem os nomes.",
            self,
            redactor,
        )
    }
}

/// Catálogo com os dados embutidos.
pub fn embedded_catalog() -> Result<AbilityIdCatalog, AbilityIdDataError> {
    parse_catalog(EMBEDDED_ABILITY_IDS)
}

/// O recorte é um objeto com o ID (em texto) como chave e o nome interno
/// como valor. Uma chave pode ter vários IDs separados por vírgula com o
/// mesmo nome, como `"3060,1617"` na fonte.
fn parse_catalog(contents: &str) -> Result<AbilityIdCatalog, AbilityIdDataError> {
    let records: HashMap<String, String> =
        serde_json::from_str(contents).map_err(AbilityIdDataError::Json)?;
    let mut names = HashMap::new();
    for (ids, name) in records {
        for id in ids.split(',') {
            let id = id
                .trim()
                .parse::<u32>()
                .map_err(|_| AbilityIdDataError::InvalidId(ids.clone()))?;
            names.insert(id, name.clone());
        }
    }
    Ok(AbilityIdCatalog::new(names))
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/ability_id_data_tests.rs"]
mod tests;
