//! Dados dos heróis embutidos no binário, a partir do recorte do dotaconstants
//! em `assets/dotaconstants/heroes.json` (fonte e licença em `SOURCE.md`).

use serde::Deserialize;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use crate::domain::hero_catalog::{
    AbilityInfo, AttackType, HeroCatalog, HeroInfo, HeroRole, PrimaryAttribute,
};

/// Conteúdo do recorte, incluído na compilação.
const EMBEDDED_HEROES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/dotaconstants/heroes.json"
));

/// O recorte embutido não pôde ser lido.
#[derive(Debug, thiserror::Error)]
#[error("os dados embutidos dos heróis não são válidos")]
pub struct HeroDataError(#[source] serde_json::Error);

impl ToDiagnostic for HeroDataError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("Dados dos heróis indisponíveis"),
            "A aba Herói mostra só o que o GSI informa.",
            self,
            redactor,
        )
    }
}

#[derive(Deserialize)]
struct HeroRecord {
    id: u32,
    name: String,
    localized_name: String,
    primary_attr: String,
    attack_type: String,
    roles: Vec<String>,
    abilities: Vec<AbilityRecord>,
    #[serde(default)]
    img: Option<String>,
}

#[derive(Deserialize)]
struct AbilityRecord {
    name: String,
    display_name: String,
    #[serde(default)]
    img: Option<String>,
}

/// Catálogo com os dados embutidos.
pub fn embedded_catalog() -> Result<HeroCatalog, HeroDataError> {
    parse_catalog(EMBEDDED_HEROES)
}

fn parse_catalog(contents: &str) -> Result<HeroCatalog, HeroDataError> {
    let records: Vec<HeroRecord> = serde_json::from_str(contents).map_err(HeroDataError)?;
    Ok(HeroCatalog::new(
        records.into_iter().map(hero_info).collect(),
    ))
}

fn hero_info(record: HeroRecord) -> HeroInfo {
    HeroInfo {
        id: record.id,
        name: record.name,
        display_name: record.localized_name,
        primary_attribute: PrimaryAttribute::from_code(&record.primary_attr),
        attack_type: AttackType::from_name(&record.attack_type),
        roles: record
            .roles
            .iter()
            .map(|role| HeroRole::from_name(role))
            .collect(),
        abilities: record
            .abilities
            .into_iter()
            .map(|ability| AbilityInfo {
                name: ability.name,
                display_name: ability.display_name,
                image_path: ability.img,
            })
            .collect(),
        image_path: record.img,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/hero_data_tests.rs"]
mod tests;
