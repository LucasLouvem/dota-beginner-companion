//! Testes unitários de `src/application/diagnostics.rs`.

use super::*;
use crate::domain::keyvalues::KeyValuesError;

const CONFIG_PATH: &str = "/lib/cfg/gamestate_integration_dota_companion.cfg";

fn read_error(kind: io::ErrorKind) -> GsiConfigFileError {
    GsiConfigFileError::Read {
        path: PathBuf::from(CONFIG_PATH),
        source: Arc::new(io::Error::new(kind, "falha simulada")),
    }
}

#[test]
fn read_without_permission_is_classified_with_path() {
    let diagnostic =
        read_error(io::ErrorKind::PermissionDenied).to_diagnostic(&Redactor::default());
    assert_eq!(
        diagnostic.code,
        DiagnosticCode::GsiConfigReadPermissionDenied
    );
    assert_eq!(
        diagnostic.cause,
        UiText::with("Sem permissão para ler {}.", [CONFIG_PATH])
    );
    assert_eq!(
        diagnostic.technical_details,
        Some(format!(
            "não foi possível ler {CONFIG_PATH}: falha simulada"
        ))
    );
}

#[test]
fn read_with_unknown_cause_is_unclassified_and_keeps_original_error() {
    let diagnostic = read_error(io::ErrorKind::InvalidData).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.title,
        UiText::new("Não foi possível ler o arquivo GSI")
    );
    assert_eq!(
        diagnostic.technical_details,
        Some(format!(
            "não foi possível ler {CONFIG_PATH}: falha simulada"
        ))
    );
}

#[test]
fn invalid_config_is_classified_with_parser_cause_in_details() {
    let invalid = GsiConfigFileError::Invalid {
        path: PathBuf::from(CONFIG_PATH),
        source: GsiConfigParseError::KeyValues(KeyValuesError::UnterminatedString(3)),
    };
    let diagnostic = invalid.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::GsiConfigInvalid);
    assert_eq!(
        diagnostic.cause,
        UiText::with("{} não está no formato KeyValues esperado.", [CONFIG_PATH])
    );
    assert_eq!(
        diagnostic.technical_details,
        Some(format!(
            "{CONFIG_PATH} não é um arquivo GSI válido: o conteúdo não é KeyValues válido: \
             texto entre aspas não terminado na linha 3"
        ))
    );
}
