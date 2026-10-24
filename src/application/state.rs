//! Estado compartilhado que a interface observa (somente leitura).

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::domain::game::GameSnapshot;
use crate::domain::match_events::MatchEvent;

use super::diagnostics::DiagnosticReport;
use super::error::{Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText};
use super::event_bus::{GsiServerError, RejectReason};
use super::ports::{ConnectionTestError, GsiRecordError, SharedError};
use super::use_cases::install_gsi_config::InstallError;

/// Operação que o núcleo executa em uma tarefa separada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreOperation {
    Diagnostics,
    InstallGsiConfig,
    ConnectionTest,
    GsiLog,
}

impl fmt::Display for CoreOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            CoreOperation::Diagnostics => "diagnóstico",
            CoreOperation::InstallGsiConfig => "instalação do arquivo GSI",
            CoreOperation::ConnectionTest => "teste de conexão",
            CoreOperation::GsiLog => "gravação do log de eventos GSI",
        })
    }
}

/// Como a tarefa terminou sem entregar o resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptionCause {
    Panicked,
    Cancelled,
    /// A tarefa parou de receber dados: o canal com o núcleo foi fechado.
    ChannelClosed,
}

impl fmt::Display for InterruptionCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            InterruptionCause::Panicked => "pânico",
            InterruptionCause::Cancelled => "cancelada",
            InterruptionCause::ChannelClosed => "canal fechado",
        })
    }
}

/// Tarefa do núcleo que terminou sem resultado. Com o aplicativo aberto isso
/// é sempre inesperado: no encerramento normal o runtime descarta as tarefas
/// sem que elas cheguem a terminar.
#[derive(Debug, Clone, thiserror::Error)]
#[error("a tarefa de {operation} foi interrompida ({cause})")]
pub struct TaskInterrupted {
    pub operation: CoreOperation,
    pub cause: InterruptionCause,
    /// Erro original da tarefa, quando existe. Não entra nos textos exibidos,
    /// porque a mensagem de um pânico pode conter dados processados.
    #[source]
    pub source: Option<SharedError>,
}

impl ToDiagnostic for TaskInterrupted {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = match self.operation {
            CoreOperation::Diagnostics => "O diagnóstico foi interrompido",
            CoreOperation::InstallGsiConfig => "A instalação do arquivo GSI foi interrompida",
            CoreOperation::ConnectionTest => "O teste de conexão foi interrompido",
            CoreOperation::GsiLog => "A gravação do log de eventos GSI parou",
        };
        let cause = match self.cause {
            InterruptionCause::Panicked => "A tarefa terminou com uma falha interna (pânico).",
            InterruptionCause::Cancelled => "A tarefa foi cancelada antes de terminar.",
            InterruptionCause::ChannelClosed => "A tarefa deixou de receber dados do núcleo.",
        };
        let resolution = match self.operation {
            CoreOperation::GsiLog => {
                "Reinicie o aplicativo para voltar a gravar; os eventos continuam aparecendo no \
                 Diagnóstico."
            }
            _ => "Tente de novo; se a falha se repetir, reinicie o aplicativo.",
        };
        Diagnostic {
            code: DiagnosticCode::CoreTaskInterrupted,
            title: UiText::new(title),
            cause: UiText::new(cause),
            resolution: UiText::new(resolution),
            technical_details: Some(redactor.redact(&self.to_string())),
        }
    }
}

/// Estado do servidor GSI local.
#[derive(Debug, Clone, Default)]
pub enum ServerState {
    #[default]
    Starting,
    Listening(SocketAddr),
    Failed(GsiServerError),
}

/// Evento GSI aceito: horário e JSON sem o bloco `auth`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedEvent {
    pub at: SystemTime,
    pub raw: Arc<str>,
}

/// Contadores e últimos eventos do coletor GSI.
#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone)]
pub enum InstallOutcome {
    Created(PathBuf),
    Unchanged(PathBuf),
    /// Já existe um arquivo diferente; precisa de confirmação do usuário.
    NeedsConfirmation(PathBuf),
    Replaced {
        path: PathBuf,
        backup: PathBuf,
    },
    Failed(InstallError),
    Interrupted(TaskInterrupted),
}

/// Estado do teste de conexão com o servidor GSI.
#[derive(Debug, Clone)]
pub enum ConnectionTest {
    Running,
    Passed,
    Failed(ConnectionTestError),
    Interrupted(TaskInterrupted),
}

/// Gravação dos eventos GSI do Dota em arquivo, ativada por configuração.
#[derive(Debug, Clone)]
pub struct GsiLogStatus {
    pub path: PathBuf,
    /// Eventos gravados com sucesso desde que o aplicativo abriu.
    pub written: u64,
    pub last_error: Option<GsiRecordError>,
    /// Presente quando a tarefa de gravação parou; nenhum evento é gravado depois.
    pub stopped: Option<TaskInterrupted>,
}

/// Evento da partida com o horário em que o aplicativo o registrou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    pub at: SystemTime,
    pub event: MatchEvent,
}

/// Estado de jogo mais recente e a linha do tempo da partida acompanhada.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchStatus {
    /// Último estado normalizado de um evento do Dota.
    pub game: Option<GameSnapshot>,
    /// Eventos da partida atual, do mais antigo ao mais recente.
    pub timeline: Vec<TimelineEntry>,
}

/// Estado completo exibido pela interface.
#[derive(Debug, Clone, Default)]
pub struct AppSnapshot {
    pub gsi: GsiStatus,
    pub match_status: MatchStatus,
    pub diagnostics: Option<DiagnosticReport>,
    /// Última rodada de diagnóstico interrompida; limpa na próxima que termina.
    pub diagnostics_interrupted: Option<TaskInterrupted>,
    pub last_install: Option<InstallOutcome>,
    /// Incrementado a cada instalação concluída, para a interface reagir uma
    /// única vez a cada resultado.
    pub install_attempt: u64,
    pub connection_test: Option<ConnectionTest>,
    /// Onde o token e a porta do aplicativo estão guardados.
    pub config_path: PathBuf,
    /// Presente quando a gravação de eventos em arquivo está ativa.
    pub gsi_log: Option<GsiLogStatus>,
    /// Remove o token dos diagnósticos exibidos.
    pub redactor: Redactor,
}
