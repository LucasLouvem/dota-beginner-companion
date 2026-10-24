//! Testes unitários de `src/infrastructure/logging.rs`.

use super::*;
use crate::application::error::DiagnosticCode;

#[test]
fn uses_directives_when_they_are_valid() {
    let (filter, error) = log_filter(Some("info"));
    assert!(error.is_none());
    assert_eq!(filter.to_string(), "info");
}

#[test]
fn shows_only_errors_when_there_are_no_directives() {
    let (filter, error) = log_filter(None);
    assert!(error.is_none());
    assert_eq!(filter.to_string(), "error");
}

#[test]
fn falls_back_to_errors_and_reports_invalid_directive() {
    let (filter, error) = log_filter(Some("info,dota=[invalido"));
    assert_eq!(filter.to_string(), "error");
    let diagnostic = error
        .expect("a diretiva inválida é informada")
        .to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(diagnostic.title, UiText::new("Nível de log inválido"));
}
