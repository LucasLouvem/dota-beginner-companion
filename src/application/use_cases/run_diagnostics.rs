//! Monta o relatório de diagnóstico: instalações do Dota, arquivo GSI e processo.

use std::time::SystemTime;

use crate::application::ports::{ConfigFiles, DotaLocator, ProcessProbe};
use crate::domain::diagnostics::{DiagnosticReport, GsiConfigState, InstallCheck};
use crate::domain::gsi_config::{self, GsiEndpoint};

/// Verifica instalações, arquivo GSI de cada uma e processo do Dota.
pub fn run_diagnostics(
    locator: &dyn DotaLocator,
    probe: &dyn ProcessProbe,
    files: &dyn ConfigFiles,
    endpoint: &GsiEndpoint,
) -> DiagnosticReport {
    let located = locator.locate();
    let installs = located
        .installs
        .into_iter()
        .map(|install| {
            let config_path = install.gsi_config_path();
            let config = match files.read(&config_path) {
                Ok(None) => GsiConfigState::Missing,
                Ok(Some(text)) => GsiConfigState::Present(gsi_config::inspect(&text, endpoint)),
                Err(io_error) => GsiConfigState::Unreadable(io_error.to_string()),
            };
            InstallCheck {
                install,
                config_path,
                config,
            }
        })
        .collect();

    DiagnosticReport {
        generated_at: SystemTime::now(),
        searched_libraries: located.searched_libraries,
        installs,
        dota_process_running: probe.dota_running(),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/run_diagnostics_tests.rs"]
mod tests;
