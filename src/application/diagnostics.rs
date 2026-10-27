//! Resultado do diagnóstico de instalação e configuração do GSI.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::domain::dota_install::DotaInstall;
use crate::domain::gsi_config::GsiConfigParseError;

use super::error::{Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText, technical_details};

/// Situação do arquivo `.cfg` do GSI em uma instalação.
#[derive(Debug, Clone)]
pub enum GsiConfigState {
    Missing,
    /// Mesmo endereço e token esperados pelo aplicativo.
    Matches,
    UriMismatch {
        found: Option<String>,
    },
    TokenMismatch,
    Failed(GsiConfigFileError),
}

/// Falha ao ler ou interpretar o arquivo `.cfg` do GSI.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GsiConfigFileError {
    #[error("não foi possível ler {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("{} não é um arquivo GSI válido", path.display())]
    Invalid {
        path: PathBuf,
        #[source]
        source: GsiConfigParseError,
    },
}

impl ToDiagnostic for GsiConfigFileError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        match self {
            GsiConfigFileError::Read { path, source } => {
                let title = UiText::new("Não foi possível ler o arquivo GSI");
                if source.kind() != io::ErrorKind::PermissionDenied {
                    return Diagnostic::unclassified(
                        title,
                        format!("Falha ao ler {}.", path.display()),
                        self,
                        redactor,
                    );
                }
                Diagnostic {
                    code: DiagnosticCode::GsiConfigReadPermissionDenied,
                    title,
                    cause: UiText::with("Sem permissão para ler {}.", [path.display().to_string()]),
                    resolution: UiText::new("Corrija as permissões da pasta do Dota."),
                    technical_details: Some(technical_details(self, redactor)),
                }
            }
            GsiConfigFileError::Invalid { path, .. } => Diagnostic {
                code: DiagnosticCode::GsiConfigInvalid,
                title: UiText::new("Arquivo GSI inválido"),
                cause: UiText::with(
                    "{} não está no formato KeyValues esperado.",
                    [path.display().to_string()],
                ),
                resolution: UiText::new(
                    "Reinstale o arquivo GSI; uma cópia de segurança é criada antes de substituir.",
                ),
                technical_details: Some(technical_details(self, redactor)),
            },
        }
    }
}

/// Instalação do Dota com o estado do seu arquivo GSI.
#[derive(Debug, Clone)]
pub struct InstallCheck {
    pub install: DotaInstall,
    pub config_path: PathBuf,
    pub config: GsiConfigState,
}

/// Resultado de uma rodada de diagnóstico. Os textos exibidos ficam na
/// camada de apresentação, para permitir tradução.
#[derive(Debug, Clone)]
pub struct DiagnosticReport {
    pub generated_at: SystemTime,
    pub searched_libraries: Vec<PathBuf>,
    pub installs: Vec<InstallCheck>,
    /// Detectado pela lista de processos (`/proc/*/comm`), sem ler memória.
    pub dota_process_running: bool,
    /// Detectado da mesma forma: só o processo `steam`.
    pub steam_process_running: bool,
}

#[cfg(test)]
#[path = "../../tests/unit/application/diagnostics_tests.rs"]
mod tests;
