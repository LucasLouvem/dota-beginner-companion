//! Testes unitários de `src/application/ports.rs`.

use super::*;

fn simulated_source(message: &str) -> SharedError {
    Arc::new(io::Error::other(message.to_owned()))
}

#[test]
fn connection_test_failures_with_known_cause_get_their_own_code() {
    let cases = [
        (
            ConnectionTestError::Unreachable {
                source: simulated_source("conexão recusada"),
            },
            DiagnosticCode::ConnectionTestUnreachable,
        ),
        (
            ConnectionTestError::TimedOut {
                source: simulated_source("tempo esgotado"),
            },
            DiagnosticCode::ConnectionTestTimedOut,
        ),
        (
            ConnectionTestError::TokenRejected,
            DiagnosticCode::ConnectionTestTokenRejected,
        ),
        (
            ConnectionTestError::PayloadRejected,
            DiagnosticCode::ConnectionTestPayloadRejected,
        ),
    ];
    for (test_error, expected_code) in cases {
        let diagnostic = test_error.to_diagnostic(&Redactor::default());
        assert_eq!(diagnostic.code, expected_code, "{test_error:?}");
        assert_eq!(diagnostic.title, UiText::new("Teste de conexão falhou"));
    }
}

#[test]
fn unreachable_keeps_original_error_in_details() {
    let diagnostic = ConnectionTestError::Unreachable {
        source: simulated_source("conexão recusada"),
    }
    .to_diagnostic(&Redactor::default());
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("o servidor GSI local não aceitou a conexão: conexão recusada")
    );
}

#[test]
fn unexpected_response_is_unclassified_with_details() {
    let diagnostic = ConnectionTestError::UnexpectedResponse {
        details: "HTTP 500 Internal Server Error".into(),
    }
    .to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("resposta inesperada do servidor GSI (HTTP 500 Internal Server Error)")
    );
}

fn record_write_error(kind: io::ErrorKind) -> GsiRecordError {
    GsiRecordError::Write {
        path: PathBuf::from("logs/gsi.jsonl"),
        source: Arc::new(io::Error::new(kind, "falha simulada")),
    }
}

#[test]
fn log_write_without_directory_is_classified_as_directory_missing() {
    let diagnostic =
        record_write_error(io::ErrorKind::NotFound).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::GsiLogDirectoryMissing);
    assert_eq!(
        diagnostic.cause,
        UiText::with("A pasta de {} não existe.", ["logs/gsi.jsonl"])
    );
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível abrir ou gravar logs/gsi.jsonl: falha simulada")
    );
}

#[test]
fn log_write_without_permission_is_classified_as_permission_denied() {
    let diagnostic =
        record_write_error(io::ErrorKind::PermissionDenied).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::GsiLogPermissionDenied);
}

#[test]
fn log_write_with_unknown_cause_is_unclassified() {
    let diagnostic =
        record_write_error(io::ErrorKind::StorageFull).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.title,
        UiText::new("Falha ao gravar o log de eventos GSI")
    );
}

#[test]
fn log_invalid_payload_is_classified_with_its_own_code() {
    let diagnostic = GsiRecordError::InvalidPayload {
        source: simulated_source("expected value at line 1 column 1"),
    }
    .to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::GsiLogInvalidPayload);
}

#[test]
fn value_is_fresh_until_ttl_and_stale_after_it() {
    let stored_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let ttl = Duration::from_secs(60);
    assert_eq!(
        Freshness::of(stored_at, stored_at + ttl, ttl),
        Freshness::Fresh
    );
    assert_eq!(
        Freshness::of(stored_at, stored_at + ttl + Duration::from_secs(1), ttl),
        Freshness::Stale
    );
}

#[test]
fn value_stored_in_the_future_counts_as_fresh() {
    let stored_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let now = stored_at - Duration::from_secs(500);
    assert_eq!(
        Freshness::of(stored_at, now, Duration::from_secs(1)),
        Freshness::Fresh
    );
}

#[test]
fn storage_operation_failure_without_known_cause_is_unclassified() {
    let storage_error = StorageError::Operation {
        operation: "ler o cache",
        kind: StorageFailureKind::Other,
        source: simulated_source("disco cheio"),
    };
    let diagnostic = storage_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("a operação \"ler o cache\" no banco local falhou: disco cheio")
    );
}

#[test]
fn request_failure_details_do_not_expose_the_token() {
    let diagnostic = ConnectionTestError::Request {
        source: simulated_source("falha ao enviar auth.token=abc123"),
    }
    .to_diagnostic(&Redactor::new(["abc123"]));
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível enviar o evento de teste: falha ao enviar auth.token=[removido]")
    );
}

#[test]
fn exhausted_budget_has_its_own_code_and_names_the_limit() {
    let budget_error = BudgetError::Exhausted(BudgetExhausted {
        category: CallCategory::Collection,
        limit: BudgetLimit::DailyCollection,
    });

    let diagnostic = budget_error.to_diagnostic(&Redactor::default());

    assert_eq!(diagnostic.code, DiagnosticCode::OpendotaBudgetExhausted);
    assert_eq!(
        diagnostic.title,
        UiText::new("Limite de consultas ao OpenDota atingido")
    );
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("o limite da coleta no dia de chamadas ao OpenDota foi atingido")
    );
}

#[test]
fn budget_storage_failure_keeps_the_storage_diagnostic() {
    let storage_error = StorageError::Operation {
        operation: "reservar uma chamada",
        kind: StorageFailureKind::Corrupted,
        source: Arc::new(io::Error::other("arquivo danificado")),
    };

    let diagnostic = BudgetError::Storage(storage_error).to_diagnostic(&Redactor::default());

    assert_eq!(diagnostic.code, DiagnosticCode::StorageCorrupted);
}
