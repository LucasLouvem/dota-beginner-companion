//! Testes unitários de `src/domain/gsi_config.rs`.

use super::*;

fn endpoint() -> GsiEndpoint {
    GsiEndpoint {
        port: 53000,
        token: "abc123".into(),
    }
}

/// Arquivo esperado para `endpoint()`, escrito à mão: loopback, sem `draft`
/// nem `wearables`.
const EXPECTED_CONFIG: &str = r#"// Gerado pelo Dota Beginner Companion.
// Envia o estado do jogo somente para este computador (127.0.0.1).
"Dota Beginner Companion"
{
    "uri" "http://127.0.0.1:53000/"
    "timeout" "5.0"
    "buffer" "0.1"
    "throttle" "0.5"
    "heartbeat" "30.0"
    "data"
    {
        "provider" "1"
        "map" "1"
        "player" "1"
        "hero" "1"
        "abilities" "1"
        "items" "1"
    }
    "auth"
    {
        "token" "abc123"
    }
}
"#;

#[test]
fn renders_config_pointing_to_loopback_without_draft() {
    assert_eq!(render(&endpoint()), EXPECTED_CONFIG);
}

#[test]
fn rendered_config_matches_its_own_endpoint() {
    assert_eq!(inspect(EXPECTED_CONFIG, &endpoint()), ConfigMatch::Matches);
}

#[test]
fn reports_token_mismatch_when_app_token_changed() {
    let config = render(&endpoint());
    let new_token = GsiEndpoint {
        token: "zzz".into(),
        ..endpoint()
    };
    assert_eq!(inspect(&config, &new_token), ConfigMatch::TokenMismatch);
}

#[test]
fn reports_uri_mismatch_with_found_uri_when_app_port_changed() {
    let config = render(&endpoint());
    let new_port = GsiEndpoint {
        port: 4000,
        ..endpoint()
    };
    assert_eq!(
        inspect(&config, &new_port),
        ConfigMatch::UriMismatch {
            found: Some("http://127.0.0.1:53000/".into())
        }
    );
}

#[test]
fn reports_unparsable_with_parser_error_when_file_is_not_keyvalues() {
    assert_eq!(
        inspect("\"broken\" {", &endpoint()),
        ConfigMatch::Unparsable(GsiConfigParseError::KeyValues(
            KeyValuesError::UnclosedSection
        ))
    );
}

#[test]
fn reports_unparsable_with_missing_section_when_file_has_only_text_values() {
    assert_eq!(
        inspect("\"uri\" \"http://127.0.0.1:53000/\"", &endpoint()),
        ConfigMatch::Unparsable(GsiConfigParseError::MissingSection)
    );
}
