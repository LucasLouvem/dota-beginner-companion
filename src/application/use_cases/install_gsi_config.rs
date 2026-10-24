//! Instala o arquivo `.cfg` do GSI em uma instalação do Dota.

use std::path::Path;

use crate::application::ports::ConfigFiles;
use crate::application::state::InstallOutcome;
use crate::domain::dota_install::DotaInstall;
use crate::domain::gsi_config::{self, ConfigMatch, GsiEndpoint};

/// Instala o `.cfg` do GSI. Nunca substitui um arquivo diferente sem
/// `overwrite`, e sempre faz backup antes de substituir.
pub fn install_gsi_config(
    files: &dyn ConfigFiles,
    install_root: &Path,
    endpoint: &GsiEndpoint,
    overwrite: bool,
) -> InstallOutcome {
    let install = DotaInstall {
        library: install_root.to_path_buf(),
        game_root: DotaInstall::game_root_for(install_root),
        native_binary: false,
        proton_prefix: false,
    };
    let path = install.gsi_config_path();
    let failed = |io_error: std::io::Error| InstallOutcome::Failed {
        path: path.clone(),
        error: io_error.to_string(),
    };

    let contents = gsi_config::render(endpoint);
    match files.read(&path) {
        Err(io_error) => failed(io_error),
        Ok(None) => match files
            .ensure_dir(&install.gsi_dir())
            .and_then(|()| files.write(&path, &contents))
        {
            Ok(()) => InstallOutcome::Created(path.clone()),
            Err(io_error) => failed(io_error),
        },
        Ok(Some(existing)) => {
            if gsi_config::inspect(&existing, endpoint) == ConfigMatch::Matches {
                return InstallOutcome::Unchanged(path.clone());
            }
            if !overwrite {
                return InstallOutcome::NeedsConfirmation(path.clone());
            }
            match files.backup(&path) {
                Err(io_error) => failed(io_error),
                Ok(backup) => match files.write(&path, &contents) {
                    Ok(()) => InstallOutcome::Replaced {
                        path: path.clone(),
                        backup,
                    },
                    Err(io_error) => failed(io_error),
                },
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/install_gsi_config_tests.rs"]
mod tests;
