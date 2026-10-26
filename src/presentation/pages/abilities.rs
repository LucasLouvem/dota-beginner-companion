//! Conteúdo da aba "Habilidades": a ordem de habilidades mais comum nos
//! primeiros níveis, em partidas profissionais válidas do herói.

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::state::RecommendationStatus;
use crate::application::use_cases::recommendations::Recommendations;
use crate::domain::ability_catalog::AbilityIdCatalog;
use crate::domain::builds::{ABILITY_ORDER_LEVELS, MIN_SAMPLE};
use crate::domain::game::GameMode;
use crate::domain::hero_catalog::HeroCatalog;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{Level, RowContent};

use super::items::{bots_note, ready_header, source_row, status_rows, turbo_warning};

/// Seções da aba: cabeçalho, avisos e a ordem de habilidades.
pub fn abilities_content(
    status: &RecommendationStatus,
    heroes: &HeroCatalog,
    abilities: &AbilityIdCatalog,
    redactor: &Redactor,
) -> Vec<Vec<RowContent>> {
    let RecommendationStatus::Ready(recommendations) = status else {
        return vec![
            status_rows(status, heroes, redactor).unwrap_or_default(),
            Vec::new(),
            Vec::new(),
        ];
    };
    vec![
        vec![ready_header(recommendations, heroes)],
        notes(recommendations.key.mode),
        order_rows(recommendations, heroes, abilities, redactor),
    ]
}

fn notes(mode: GameMode) -> Vec<RowContent> {
    let mut notes = vec![RowContent::new(
        Level::Info,
        &translate("A ordem não considera posição."),
        &translate("A posição (1 a 5) não vem do Dota, e as partidas misturam todas as posições."),
    )];
    match mode {
        GameMode::Turbo => notes.push(turbo_warning(&translate(
            "No turbo os níveis chegam mais rápido, mas a ordem das habilidades segue valendo como referência.",
        ))),
        GameMode::Bots => notes.push(bots_note()),
        GameMode::AllPick => {}
    }
    notes
}

fn order_rows(
    recommendations: &Recommendations,
    heroes: &HeroCatalog,
    abilities: &AbilityIdCatalog,
    redactor: &Redactor,
) -> Vec<RowContent> {
    let pro = &recommendations.pro;
    let mut rows = match &pro.ability_order {
        Some(order) => {
            let sample = translate_with(
                "Mais comum: {} de {} partidas profissionais válidas",
                &[&order.matches.to_string(), &order.analyzed.to_string()],
            );
            order
                .ability_ids
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    RowContent::new(
                        Level::Info,
                        &translate_with(
                            "Nível {}: {}",
                            &[
                                &(index + 1).to_string(),
                                &ability_name(*id, heroes, abilities),
                            ],
                        ),
                        &sample,
                    )
                })
                .collect()
        }
        None => vec![RowContent::new(
            Level::Info,
            &translate("Amostra insuficiente"),
            &translate_with(
                "Nenhuma ordem dos {} primeiros níveis aparece em pelo menos {} partidas entre {} partidas profissionais válidas.",
                &[
                    &ABILITY_ORDER_LEVELS.to_string(),
                    &MIN_SAMPLE.to_string(),
                    &pro.valid_matches.to_string(),
                ],
            ),
        )],
    };
    rows.push(source_row(
        &translate_with(
            "Fonte: OpenDota, partidas profissionais de liga · patch {}",
            &[&recommendations.patch.name],
        ),
        pro.candidates_origin,
    ));
    if let Some(error) = &pro.error {
        rows.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    rows
}

/// Nome de exibição da habilidade; talentos aparecem como "Talento", e um
/// ID desconhecido, como o próprio número.
pub fn ability_name(id: u32, heroes: &HeroCatalog, abilities: &AbilityIdCatalog) -> String {
    match abilities.name(id) {
        Some(name) if name.starts_with("special_bonus") => translate("Talento"),
        Some(name) => heroes.ability_display_name(name).unwrap_or(name).to_owned(),
        None => translate_with("habilidade {}", &[&id.to_string()]),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/abilities_tests.rs"]
mod tests;
