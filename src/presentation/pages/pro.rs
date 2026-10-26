//! Conteúdo da aba "Dados profissionais": partidas profissionais recentes
//! com o herói.

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::state::RecommendationStatus;
use crate::application::use_cases::recommendations::Recommendations;
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::match_data::HeroMatch;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{Level, RowContent, format_date, format_duration};

use super::items::{ready_header, source_row, status_rows};

/// Partidas mostradas, das mais recentes.
pub const RECENT_MATCHES_SHOWN: usize = 10;

/// Seções da aba: cabeçalho e partidas recentes.
pub fn pro_content(
    status: &RecommendationStatus,
    heroes: &HeroCatalog,
    redactor: &Redactor,
) -> Vec<Vec<RowContent>> {
    let RecommendationStatus::Ready(recommendations) = status else {
        return vec![
            status_rows(status, heroes, redactor).unwrap_or_default(),
            Vec::new(),
        ];
    };
    vec![
        vec![ready_header(recommendations, heroes)],
        match_rows(recommendations, redactor),
    ]
}

fn match_rows(recommendations: &Recommendations, redactor: &Redactor) -> Vec<RowContent> {
    let pro = &recommendations.pro;
    let mut league_matches: Vec<&HeroMatch> = pro
        .recent_matches
        .iter()
        .filter(|recent| recent.is_league_match())
        .collect();
    league_matches.sort_by_key(|recent| std::cmp::Reverse(recent.start_time));
    let mut rows: Vec<RowContent> = league_matches
        .into_iter()
        .take(RECENT_MATCHES_SHOWN)
        .map(match_row)
        .collect();
    if rows.is_empty() {
        rows.push(RowContent::new(
            Level::Info,
            &translate("Nenhuma partida profissional recente do herói"),
            &translate("O OpenDota não trouxe partidas de liga com este herói."),
        ));
    }
    rows.push(source_row(
        &translate("Fonte: OpenDota, partidas recentes de liga com o herói"),
        pro.candidates_origin,
    ));
    if let Some(error) = &pro.error {
        rows.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    rows
}

/// Linha de uma partida: liga e resultado do herói, data e duração.
pub fn match_row(recent: &HeroMatch) -> RowContent {
    let league = recent
        .league_name
        .clone()
        .unwrap_or_else(|| translate("Liga sem nome"));
    let result = match recent.hero_won {
        Some(true) => translate("vitória"),
        Some(false) => translate("derrota"),
        None => translate("resultado não informado"),
    };
    let mut details = Vec::new();
    if let Some(start_time) = recent.start_time {
        details.push(format_date(start_time));
    }
    if let Some(duration) = recent.duration_secs {
        details.push(translate_with("duração {}", &[&format_duration(duration)]));
    }
    RowContent::new(
        Level::Info,
        &translate_with("{}: {}", &[&league, &result]),
        &details.join(" · "),
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/pro_tests.rs"]
mod tests;
