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
fn keeps_extra_placeholders_when_arguments_run_out() {
    assert_eq!(translate_with("{} e {}", &["1"]), "1 e {}");
}
