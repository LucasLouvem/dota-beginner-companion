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
