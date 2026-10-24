//! Resultado do diagnóstico de instalação e configuração do GSI.

use std::path::PathBuf;
use std::time::SystemTime;

use super::dota_install::DotaInstall;
use super::gsi_config::ConfigMatch;

/// Situação do arquivo `.cfg` do GSI em uma instalação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GsiConfigState {
    Missing,
    Present(ConfigMatch),
    Unreadable(String),
}

/// Instalação do Dota com o estado do seu arquivo GSI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCheck {
    pub install: DotaInstall,
    pub config_path: PathBuf,
    pub config: GsiConfigState,
}

/// Resultado de uma rodada de diagnóstico. Os textos exibidos ficam na
/// camada de apresentação, para permitir tradução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticReport {
    pub generated_at: SystemTime,
    pub searched_libraries: Vec<PathBuf>,
    pub installs: Vec<InstallCheck>,
    /// Detectado pela lista de processos (`/proc/*/comm`), sem ler memória.
    pub dota_process_running: bool,
}
