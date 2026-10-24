//! Testes unitários de `src/domain/keyvalues.rs`.

use super::*;

#[test]
fn reads_nested_sections_ignoring_comments_and_key_case() {
    let document = r#"
        // comentário
        "root"
        {
            "name"   "1"
            "child" { "inner" "two words" }
            unquoted value
        }
    "#;
    let parsed = parse(document).unwrap();
    let root = parsed.get("root").unwrap();
    assert_eq!(root.get("name").unwrap().as_text(), Some("1"));
    assert_eq!(
        root.get("CHILD").unwrap().get("inner").unwrap().as_text(),
        Some("two words")
    );
    assert_eq!(root.get("unquoted").unwrap().as_text(), Some("value"));
}

#[test]
fn unescapes_backslashes_and_quotes_inside_text() {
    let parsed = parse(r#""path" "C:\\Games\\\"x\"""#).unwrap();
    assert_eq!(
        parsed.get("path").unwrap().as_text(),
        Some(r#"C:\Games\"x""#)
    );
}

#[test]
fn fails_when_section_is_never_closed() {
    assert_eq!(
        parse(r#""root" { "key" "value""#),
        Err(KeyValuesError::UnclosedSection)
    );
}

#[test]
fn fails_with_line_when_closing_brace_has_no_section() {
    assert_eq!(
        parse(r#""key" "value" }"#),
        Err(KeyValuesError::UnexpectedClose(1))
    );
}

#[test]
fn fails_with_line_when_key_has_no_value() {
    assert_eq!(parse(r#""key""#), Err(KeyValuesError::MissingValue(1)));
}

#[test]
fn fails_with_line_when_quote_is_not_closed() {
    assert_eq!(parse("\"key"), Err(KeyValuesError::UnterminatedString(1)));
}
