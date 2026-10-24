//! Testes unitários de `src/application/event_bus.rs`.

use super::*;
use std::net::{Ipv4Addr, SocketAddr};

fn bind_error(kind: io::ErrorKind) -> GsiServerError {
    GsiServerError::Bind {
        address: SocketAddr::from((Ipv4Addr::LOCALHOST, 53000)),
        source: Arc::new(io::Error::new(kind, "falha simulada")),
    }
}

#[test]
fn bind_with_port_in_use_is_classified_as_gsi_bind_failed() {
    let diagnostic = bind_error(io::ErrorKind::AddrInUse).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::GsiBindFailed);
    assert_eq!(diagnostic.title, UiText::new("Servidor GSI não iniciou"));
    assert_eq!(
        diagnostic.cause,
        UiText::with("A porta {} já está sendo usada.", ["53000"])
    );
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível escutar em 127.0.0.1:53000: falha simulada")
    );
}

#[test]
fn bind_without_permission_is_classified_as_permission_denied() {
    let diagnostic =
        bind_error(io::ErrorKind::PermissionDenied).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::GsiBindPermissionDenied);
    assert_eq!(
        diagnostic.cause,
        UiText::with("Sem permissão para escutar na porta {}.", ["53000"])
    );
}

#[test]
fn bind_with_unknown_cause_is_unclassified_and_keeps_original_error() {
    let diagnostic =
        bind_error(io::ErrorKind::AddrNotAvailable).to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.cause,
        UiText::with(
            "{} A causa ainda não foi classificada.",
            ["Falha ao escutar em 127.0.0.1:53000."]
        )
    );
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível escutar em 127.0.0.1:53000: falha simulada")
    );
}

#[test]
fn serve_failure_is_unclassified_with_server_stopped_title() {
    let serve_error = GsiServerError::Serve {
        source: Arc::new(io::Error::other("conexão encerrada")),
    };
    let diagnostic = serve_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(diagnostic.title, UiText::new("Servidor GSI parou"));
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("o servidor GSI parou de atender requisições: conexão encerrada")
    );
}

#[test]
fn rejected_command_is_core_unavailable_without_command_data() {
    let rejected = CoreConnectionError::CommandRejected {
        command: UiCommand::InstallGsiConfig {
            install_root: PathBuf::from("/home/usuario/SteamLibrary"),
            overwrite: true,
        },
    };
    let diagnostic = rejected.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::CoreUnavailable);
    assert_eq!(
        diagnostic.cause,
        UiText::new("O pedido para instalar a configuração GSI não foi recebido.")
    );
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("o núcleo não recebeu o comando de instalar a configuração GSI")
    );
}

#[test]
fn closed_state_is_core_unavailable() {
    let diagnostic = CoreConnectionError::StateClosed.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::CoreUnavailable);
    assert_eq!(
        diagnostic.title,
        UiText::new("O núcleo do aplicativo parou")
    );
    assert_eq!(diagnostic.resolution, UiText::new("Reinicie o aplicativo."));
}

#[test]
fn bind_error_keeps_original_io_error_as_source() {
    let server_error = bind_error(io::ErrorKind::AddrInUse);
    let source = std::error::Error::source(&server_error).expect("o erro de bind tem causa");
    let io_error = source
        .downcast_ref::<Arc<io::Error>>()
        .expect("a causa é o erro de I/O original");
    assert_eq!(io_error.kind(), io::ErrorKind::AddrInUse);
}
