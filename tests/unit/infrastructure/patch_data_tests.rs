//! Testes unitários de `src/infrastructure/patch_data.rs`.

use super::*;

#[test]
fn embedded_current_patch_is_the_last_of_the_snapshot() {
    let catalog = embedded_catalog().expect("o recorte embutido é válido");
    assert_eq!(
        catalog.current(),
        Some(&Patch {
            id: 60,
            name: "7.41".into()
        })
    );
}

#[test]
fn reads_id_and_name_and_ignores_the_date() {
    let catalog =
        parse_catalog(r#"[{"name": "7.40", "date": "2025-12-16T00:50:40.281Z", "id": 59}]"#)
            .unwrap();
    assert_eq!(
        catalog.by_id(59).map(|patch| patch.name.as_str()),
        Some("7.40")
    );
}

#[test]
fn fails_with_typed_error_when_a_patch_has_no_id() {
    let error = parse_catalog(r#"[{"name": "7.40"}]"#).unwrap_err();
    let diagnostic = error.to_diagnostic(&Redactor::default());
    assert_eq!(
        diagnostic.title,
        UiText::new("Dados dos patches indisponíveis")
    );
}
