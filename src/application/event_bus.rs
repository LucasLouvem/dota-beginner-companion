//! Mensagens trocadas entre adaptadores, núcleo e interface.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

/// Eventos produzidos pelos adaptadores de entrada (servidor GSI).
#[derive(Debug, Clone)]
pub enum CoreEvent {
    ServerListening(SocketAddr),
    ServerFailed(String),
    GsiAccepted {
        raw: Arc<str>,
        at: SystemTime,
        /// Evento enviado pelo botão "Testar conexão", não pelo Dota.
        is_test: bool,
    },
    GsiRejected {
        reason: RejectReason,
        at: SystemTime,
    },
}

/// Motivo de uma requisição GSI recusada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    InvalidToken,
    InvalidJson,
}

/// Comandos enviados pela interface ao núcleo.
#[derive(Debug, Clone)]
pub enum UiCommand {
    RunDiagnostics,
    TestConnection,
    InstallGsiConfig {
        install_root: PathBuf,
        /// O usuário confirmou a substituição de um arquivo diferente.
        overwrite: bool,
    },
}
