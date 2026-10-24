//! Mensagens trocadas entre adaptadores, núcleo e interface.

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::domain::game::GameSnapshot;

use super::error::{Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText, technical_details};

/// Eventos produzidos pelos adaptadores de entrada (servidor GSI).
#[derive(Debug, Clone)]
pub enum CoreEvent {
    ServerListening(SocketAddr),
    ServerFailed(GsiServerError),
    GsiAccepted {
        raw: Arc<str>,
        at: SystemTime,
        /// Evento enviado pelo botão "Testar conexão", não pelo Dota.
        is_test: bool,
        /// Estado de jogo normalizado; ausente nos eventos de teste.
        game: Option<GameSnapshot>,
    },
    GsiRejected {
        reason: RejectReason,
        at: SystemTime,
    },
}

/// Falha do servidor GSI local. O `Arc` mantém o erro original e permite
/// copiá-lo para o estado da interface.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GsiServerError {
    #[error("não foi possível escutar em {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("o servidor GSI parou de atender requisições")]
    Serve {
        #[source]
        source: Arc<io::Error>,
    },
}

impl ToDiagnostic for GsiServerError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Servidor GSI não iniciou");
        match self {
            GsiServerError::Bind { address, source } => match source.kind() {
                io::ErrorKind::AddrInUse => Diagnostic {
                    code: DiagnosticCode::GsiBindFailed,
                    title,
                    cause: UiText::with(
                        "A porta {} já está sendo usada.",
                        [address.port().to_string()],
                    ),
                    resolution: UiText::new(
                        "Escolha outra porta em gsi_port na configuração e reinstale o arquivo GSI.",
                    ),
                    technical_details: Some(technical_details(self, redactor)),
                },
                io::ErrorKind::PermissionDenied => Diagnostic {
                    code: DiagnosticCode::GsiBindPermissionDenied,
                    title,
                    cause: UiText::with(
                        "Sem permissão para escutar na porta {}.",
                        [address.port().to_string()],
                    ),
                    resolution: UiText::new(
                        "Portas abaixo de 1024 exigem privilégios no Linux. Escolha outra porta em \
                         gsi_port e reinstale o arquivo GSI.",
                    ),
                    technical_details: Some(technical_details(self, redactor)),
                },
                _ => Diagnostic::unclassified(
                    title,
                    format!("Falha ao escutar em {address}."),
                    self,
                    redactor,
                ),
            },
            GsiServerError::Serve { .. } => Diagnostic::unclassified(
                UiText::new("Servidor GSI parou"),
                "O servidor GSI deixou de atender requisições.",
                self,
                redactor,
            ),
        }
    }
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
    /// O usuário marcou "Partida encontrada", sem depender do GSI.
    MarkMatchFound,
}

impl UiCommand {
    /// Nome curto do comando, sem os dados que ele carrega (como caminhos).
    pub fn describe(&self) -> &'static str {
        match self {
            UiCommand::RunDiagnostics => "executar o diagnóstico",
            UiCommand::TestConnection => "testar a conexão",
            UiCommand::InstallGsiConfig { .. } => "instalar a configuração GSI",
            UiCommand::MarkMatchFound => "marcar a partida encontrada",
        }
    }
}

/// Um dos lados perdeu o canal com o núcleo. Com o aplicativo aberto isso é
/// sempre inesperado: o núcleo só para quando o runtime é encerrado, depois
/// que a janela fecha.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreConnectionError {
    #[error("o núcleo parou de publicar o estado")]
    StateClosed,
    #[error("o núcleo não recebeu o comando de {}", command.describe())]
    CommandRejected { command: UiCommand },
    #[error("o núcleo não recebeu o evento do servidor GSI")]
    EventRejected,
}

impl ToDiagnostic for CoreConnectionError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let cause = match self {
            CoreConnectionError::StateClosed => {
                "A tela deixou de receber atualizações e pode estar desatualizada."
            }
            CoreConnectionError::CommandRejected { command } => match command {
                UiCommand::RunDiagnostics => {
                    "O pedido para executar o diagnóstico não foi recebido."
                }
                UiCommand::TestConnection => "O pedido para testar a conexão não foi recebido.",
                UiCommand::InstallGsiConfig { .. } => {
                    "O pedido para instalar a configuração GSI não foi recebido."
                }
                UiCommand::MarkMatchFound => {
                    "O pedido para marcar a partida encontrada não foi recebido."
                }
            },
            CoreConnectionError::EventRejected => {
                "Um evento do servidor GSI não pôde ser entregue ao núcleo."
            }
        };
        Diagnostic {
            code: DiagnosticCode::CoreUnavailable,
            title: UiText::new("O núcleo do aplicativo parou"),
            cause: UiText::new(cause),
            resolution: UiText::new("Reinicie o aplicativo."),
            technical_details: Some(redactor.redact(&self.to_string())),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/application/event_bus_tests.rs"]
mod tests;
