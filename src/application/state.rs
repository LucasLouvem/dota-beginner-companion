//! Estado compartilhado que a interface observa (somente leitura).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::domain::diagnostics::DiagnosticReport;

use super::event_bus::RejectReason;
use super::ports::GsiRecordError;

/// Estado do servidor GSI local.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ServerState {
    #[default]
    Starting,
    Listening(SocketAddr),
    Failed(String),
}

/// Evento GSI aceito: horário e JSON sem o bloco `auth`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedEvent {
    pub at: SystemTime,
    pub raw: Arc<str>,
}

/// Contadores e últimos eventos do coletor GSI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GsiStatus {
    pub server: ServerState,
    /// Eventos aceitos vindos do Dota (testes manuais não contam).
    pub dota_events: u64,
    pub last_dota_event: Option<ReceivedEvent>,
    pub last_test_event: Option<ReceivedEvent>,
    pub rejected: u64,
    pub last_rejection: Option<(SystemTime, RejectReason)>,
}

/// Resultado da instalação do arquivo `.cfg` do GSI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    Created(PathBuf),
    Unchanged(PathBuf),
    /// Já existe um arquivo diferente; precisa de confirmação do usuário.
    NeedsConfirmation(PathBuf),
    Replaced {
        path: PathBuf,
        backup: PathBuf,
    },
    Failed {
        path: PathBuf,
        error: String,
    },
}

/// Estado do teste de conexão com o servidor GSI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionTest {
    Running,
    Passed,
    Failed(String),
}

/// Gravação dos eventos GSI do Dota em arquivo, ativada por configuração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsiLogStatus {
    pub path: PathBuf,
    /// Eventos gravados com sucesso desde que o aplicativo abriu.
    pub written: u64,
    pub last_error: Option<GsiRecordError>,
}

/// Estado completo exibido pela interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppSnapshot {
    pub gsi: GsiStatus,
    pub diagnostics: Option<DiagnosticReport>,
    pub last_install: Option<InstallOutcome>,
    /// Incrementado a cada instalação concluída, para a interface reagir uma
    /// única vez a cada resultado.
    pub install_attempt: u64,
    pub connection_test: Option<ConnectionTest>,
    /// Onde o token e a porta do aplicativo estão guardados.
    pub config_path: PathBuf,
    /// Presente quando a gravação de eventos em arquivo está ativa.
    pub gsi_log: Option<GsiLogStatus>,
}
