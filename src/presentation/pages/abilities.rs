//! Conteúdo da aba "Habilidades": a ordem real em que as habilidades foram
//! evoluídas em cada vitória profissional recente do herói.

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::state::RecommendationStatus;
use crate::application::use_cases::pro_builds::{PRO_WINS_SHOWN, ProWin};
use crate::application::use_cases::recommendations::Recommendations;
use crate::domain::ability_catalog::AbilityIdCatalog;
use crate::domain::game::GameMode;
use crate::domain::hero_catalog::HeroCatalog;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{Level, RowContent};

use super::items::{
    bots_note, examined_note, found_row, pro_win_heading, ready_header, source_row, status_rows,
    turbo_warning,
};

/// Seções da aba: cabeçalho, avisos e a ordem de habilidades de cada vitória.
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
    let mut rows: Vec<RowContent> = pro
        .wins
        .iter()
        .map(|win| order_row(win, heroes, abilities))
        .collect();
    if let Some(found) = found_row(pro.wins.len(), PRO_WINS_SHOWN, &examined_note(pro.examined)) {
        rows.push(found);
    }
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

/// Linha de uma vitória: a liga e a data, e cada nível com a habilidade
/// evoluída, na ordem da partida.
pub fn order_row(win: &ProWin, heroes: &HeroCatalog, abilities: &AbilityIdCatalog) -> RowContent {
    let (heading, detail) = pro_win_heading(win);
    let order = match &win.ability_upgrades {
        Some(upgrades) if !upgrades.is_empty() => upgrades
            .iter()
            .enumerate()
            .map(|(index, id)| format!("{} {}", index + 1, ability_name(*id, heroes, abilities)))
            .collect::<Vec<_>>()
            .join(" → "),
        _ => translate("Sem ordem de habilidades: a partida não foi analisada pelo OpenDota."),
    };
    let subtitle = if detail.is_empty() {
        order
    } else {
        format!("{detail} · {order}")
    };
    RowContent::new(Level::Info, &heading, &subtitle)
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
