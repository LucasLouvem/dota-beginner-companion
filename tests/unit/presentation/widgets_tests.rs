//! Testes unitários de `src/presentation/widgets.rs`.

use super::*;
use crate::application::error::{DiagnosticCode, UiText};

fn diagnostic(technical_details: Option<&str>) -> Diagnostic {
    Diagnostic {
        code: DiagnosticCode::Unclassified,
        title: UiText::new("Servidor GSI parou"),
        cause: UiText::with("Falha em {}.", ["127.0.0.1:53000"]),
        resolution: UiText::new("Consulte os detalhes técnicos."),
        technical_details: technical_details.map(str::to_owned),
    }
}

#[test]
fn diagnostic_subtitle_shows_cause_resolution_code_and_details() {
    assert_eq!(
        diagnostic_subtitle(&diagnostic(Some("endereço em uso"))),
        "Falha em 127.0.0.1:53000. Consulte os detalhes técnicos.\n\
         Código: UNCLASSIFIED_ERROR\n\
         Detalhes técnicos: endereço em uso"
    );
}

#[test]
fn diagnostic_text_starts_with_title_line() {
    assert_eq!(
        diagnostic_text(&diagnostic(None)),
        "Servidor GSI parou\n\
         Falha em 127.0.0.1:53000. Consulte os detalhes técnicos.\n\
         Código: UNCLASSIFIED_ERROR"
    );
}

#[test]
fn diagnostic_subtitle_omits_details_line_when_there_are_none() {
    assert_eq!(
        diagnostic_subtitle(&diagnostic(None)),
        "Falha em 127.0.0.1:53000. Consulte os detalhes técnicos.\nCódigo: UNCLASSIFIED_ERROR"
    );
}

#[test]
fn diagnostic_row_content_is_an_error_with_title_and_full_subtitle() {
    assert_eq!(
        RowContent::diagnostic(&diagnostic(Some("endereço em uso"))),
        RowContent {
            level: Level::Error,
            title: "Servidor GSI parou".into(),
            subtitle: "Falha em 127.0.0.1:53000. Consulte os detalhes técnicos.\n\
                       Código: UNCLASSIFIED_ERROR\n\
                       Detalhes técnicos: endereço em uso"
                .into(),
        }
    );
}

fn row(level: Level, title: &str) -> RowContent {
    RowContent::new(level, title, "Confirmado pelo GSI")
}

#[test]
fn row_with_a_different_level_is_replaced() {
    let shown = vec![row(Level::Warning, "Herói sem dados embutidos: x")];
    let new = vec![row(Level::Ok, "Zeus")];
    assert_eq!(row_changes(&shown, &new), vec![RowChange::Replace(0)]);
}

#[test]
fn longer_list_appends_only_the_new_rows() {
    let shown = vec![row(Level::Info, "Nível: 1")];
    let new = vec![
        row(Level::Info, "Nível: 1"),
        row(Level::Info, "Ouro: 600"),
        row(Level::Info, "GPM: 300"),
    ];
    assert_eq!(
        row_changes(&shown, &new),
        vec![RowChange::Append(1), RowChange::Append(2)]
    );
}

#[test]
fn shorter_list_truncates_and_updates_the_changed_row() {
    let shown = vec![
        row(Level::Info, "Satanic"),
        row(Level::Info, "Reaver"),
        row(Level::Info, "Claymore"),
    ];
    let new = vec![row(Level::Info, "Nenhum item")];
    assert_eq!(
        row_changes(&shown, &new),
        vec![RowChange::Update(0), RowChange::Truncate(1)]
    );
}

#[test]
fn duration_uses_hours_only_when_needed() {
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(1429), "23:49");
    assert_eq!(format_duration(3723), "1:02:03");
}
