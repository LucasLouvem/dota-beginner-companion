//! Testes unitários de `src/presentation/i18n.rs`.

use super::*;

#[test]
fn replaces_each_placeholder_with_the_next_argument() {
    assert_eq!(translate_with("{} de {}", &["1", "2"]), "1 de 2");
}

#[test]
fn keeps_text_unchanged_when_there_are_no_placeholders() {
    assert_eq!(translate_with("sem marcador", &["x"]), "sem marcador");
}

#[test]
fn translates_ui_text_replacing_its_arguments() {
    let text = UiText::with("A porta {} já está sendo usada.", ["53000"]);
    assert_eq!(translate_text(&text), "A porta 53000 já está sendo usada.");
}

#[test]
fn unsupported_locale_is_reported_as_translations_unavailable() {
    let diagnostic = TranslationSetupError::LocaleUnsupported.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::TranslationsUnavailable);
    assert_eq!(
        diagnostic.resolution,
        UiText::new("Verifique se o idioma definido em LANG está instalado no sistema.")
    );
}

#[test]
fn failed_domain_binding_keeps_original_error_in_details() {
    let setup_error = TranslationSetupError::BindDomain(io::Error::other("falha simulada"));
    let diagnostic = setup_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::TranslationsUnavailable);
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível indicar a pasta das traduções: falha simulada")
    );
}

#[test]
fn keeps_extra_placeholders_when_arguments_run_out() {
    assert_eq!(translate_with("{} e {}", &["1"]), "1 e {}");
}
