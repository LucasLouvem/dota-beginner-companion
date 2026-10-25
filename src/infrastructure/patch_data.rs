//! Patches do Dota embutidos no binário, a partir do recorte do dotaconstants
//! em `assets/dotaconstants/patch.json` (fonte e licença em `SOURCE.md`).

use serde::Deserialize;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use crate::domain::patch::{Patch, PatchCatalog};

/// Conteúdo do recorte, incluído na compilação.
const EMBEDDED_PATCHES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/dotaconstants/patch.json"
));

/// O recorte embutido não pôde ser lido.
#[derive(Debug, thiserror::Error)]
#[error("os dados embutidos dos patches não são válidos")]
pub struct PatchDataError(#[source] serde_json::Error);

impl ToDiagnostic for PatchDataError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("Dados dos patches indisponíveis"),
            "O patch atual só aparece com os dados do OpenDota.",
            self,
            redactor,
        )
    }
}

/// Um registro da fonte; a data de lançamento não é usada.
#[derive(Deserialize)]
struct PatchRecord {
    id: u32,
    name: String,
}

/// Catálogo com os dados embutidos.
pub fn embedded_catalog() -> Result<PatchCatalog, PatchDataError> {
    parse_catalog(EMBEDDED_PATCHES)
}

/// Lê uma lista de patches no formato do dotaconstants.
fn parse_catalog(contents: &str) -> Result<PatchCatalog, PatchDataError> {
    let records: Vec<PatchRecord> = serde_json::from_str(contents).map_err(PatchDataError)?;
    Ok(PatchCatalog::new(
        records
            .into_iter()
            .map(|record| Patch {
                id: record.id,
                name: record.name,
            })
            .collect(),
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/patch_data_tests.rs"]
mod tests;
