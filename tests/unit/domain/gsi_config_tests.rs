//! Testes unitários de `src/domain/gsi_config.rs`.

use super::*;

fn endpoint() -> GsiEndpoint {
    GsiEndpoint {
        port: 53000,
        token: "abc123".into(),
    }
}

#[test]
fn rendered_config_points_to_loopback_and_matches_its_endpoint() {
    let config = render(&endpoint());
    assert!(config.contains("\"uri\" \"http://127.0.0.1:53000/\""));
    assert!(!config.contains("draft"));
    assert_eq!(inspect(&config, &endpoint()), ConfigMatch::Matches);
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
fn reports_unparsable_when_file_is_not_keyvalues() {
    assert!(matches!(
        inspect("\"broken\" {", &endpoint()),
        ConfigMatch::Unparsable(_)
    ));
}
