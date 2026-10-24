//! Testes unitários de `src/application/error.rs`.

use super::*;

/// Erro com mensagem fixa e causa opcional, para montar cadeias controladas.
#[derive(Debug)]
struct ChainedError {
    message: &'static str,
    source: Option<Box<ChainedError>>,
}

impl fmt::Display for ChainedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ChainedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

fn chained(message: &'static str, source: Option<ChainedError>) -> ChainedError {
    ChainedError {
        message,
        source: source.map(Box::new),
    }
}

#[test]
fn every_code_is_screaming_snake_case_and_unique() {
    let codes: Vec<&str> = DiagnosticCode::ALL
        .iter()
        .map(|code| code.as_str())
        .collect();
    for code in &codes {
        assert!(!code.is_empty());
        assert!(
            code.chars()
                .all(|character| character.is_ascii_uppercase() || character == '_'),
            "{code}"
        );
    }
    let mut unique = codes.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), codes.len());
}

#[test]
fn unclassified_code_is_reserved_name() {
    assert_eq!(DiagnosticCode::Unclassified.as_str(), "UNCLASSIFIED_ERROR");
}

#[test]
fn technical_details_include_every_cause_in_order() {
    let error = chained(
        "falha ao gravar",
        Some(chained("disco cheio", Some(chained("erro 28", None)))),
    );
    assert_eq!(
        technical_details(&error, &Redactor::default()),
        "falha ao gravar: disco cheio: erro 28"
    );
}

#[test]
fn technical_details_remove_secret_found_in_any_cause() {
    let error = chained(
        "requisição recusada",
        Some(chained("token abc123 inválido", None)),
    );
    let redactor = Redactor::new(["abc123"]);
    assert_eq!(
        technical_details(&error, &redactor),
        "requisição recusada: token [removido] inválido"
    );
}

#[test]
fn redactor_ignores_empty_secrets() {
    assert_eq!(Redactor::new([""]).redact("texto"), "texto");
}

#[test]
fn redactor_debug_output_does_not_show_secrets() {
    let debug_output = format!("{:?}", Redactor::new(["abc123"]));
    assert_eq!(debug_output, "Redactor { secrets: 1 }");
}

#[test]
fn logged_error_has_code_and_sanitized_details_only() {
    let (_guard, logs) = crate::test_support::capture_logs();
    let error = chained("falha", Some(chained("token abc123", None)));
    let diagnostic = Diagnostic::unclassified(
        UiText::new("Título"),
        "Contexto.",
        &error,
        &Redactor::new(["abc123"]),
    );

    log_error(&diagnostic, "operation failed");

    let text = logs.text();
    assert!(text.contains("ERROR"), "{text}");
    assert!(text.contains("operation failed"), "{text}");
    assert!(text.contains("code=UNCLASSIFIED_ERROR"), "{text}");
    assert!(text.contains("falha: token [removido]"), "{text}");
    assert!(!text.contains("abc123"), "{text}");
}

#[test]
fn logged_warning_uses_warn_level() {
    let (_guard, logs) = crate::test_support::capture_logs();
    let diagnostic = Diagnostic {
        code: DiagnosticCode::TranslationsUnavailable,
        title: UiText::new("Título"),
        cause: UiText::new("Causa."),
        resolution: UiText::new("Resolução."),
        technical_details: None,
    };

    log_warning(&diagnostic, "using fallback");

    let text = logs.text();
    assert!(text.contains("WARN"), "{text}");
    assert!(text.contains("code=TRANSLATIONS_UNAVAILABLE"), "{text}");
}

#[test]
fn unclassified_diagnostic_keeps_context_and_original_error() {
    let error = chained("conexão perdida", Some(chained("token abc123", None)));
    let diagnostic = Diagnostic::unclassified(
        UiText::new("Servidor GSI parou"),
        "Contexto com abc123.",
        &error,
        &Redactor::new(["abc123"]),
    );
    assert_eq!(
        diagnostic,
        Diagnostic {
            code: DiagnosticCode::Unclassified,
            title: UiText::new("Servidor GSI parou"),
            cause: UiText::with(
                "{} A causa ainda não foi classificada.",
                ["Contexto com [removido]."]
            ),
            resolution: UiText::new("Consulte os detalhes técnicos para investigar a falha."),
            technical_details: Some("conexão perdida: token [removido]".into()),
        }
    );
}
