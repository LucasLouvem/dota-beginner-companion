//! Monta o relatório de diagnóstico: instalações do Dota, arquivo GSI e processo.

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::application::diagnostics::{
    DiagnosticReport, GsiConfigFileError, GsiConfigState, InstallCheck,
};
use crate::application::ports::{ConfigFiles, DotaLocator, ProcessProbe};
use crate::domain::gsi_config::{self, ConfigMatch, GsiEndpoint};

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
            let config = config_state(files, &config_path, endpoint);
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

fn config_state(files: &dyn ConfigFiles, path: &Path, endpoint: &GsiEndpoint) -> GsiConfigState {
    let contents = match files.read(path) {
        Ok(None) => return GsiConfigState::Missing,
        Ok(Some(contents)) => contents,
        Err(io_error) => {
            return GsiConfigState::Failed(GsiConfigFileError::Read {
                path: path.to_path_buf(),
                source: Arc::new(io_error),
            });
        }
    };
    match gsi_config::inspect(&contents, endpoint) {
        ConfigMatch::Matches => GsiConfigState::Matches,
        ConfigMatch::UriMismatch { found } => GsiConfigState::UriMismatch { found },
        ConfigMatch::TokenMismatch => GsiConfigState::TokenMismatch,
        ConfigMatch::Unparsable(parse_error) => {
            GsiConfigState::Failed(GsiConfigFileError::Invalid {
                path: path.to_path_buf(),
                source: parse_error,
            })
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/run_diagnostics_tests.rs"]
mod tests;
