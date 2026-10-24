//! Instala o arquivo `.cfg` do GSI em uma instalação do Dota.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::error::{
    Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText, technical_details,
};
use crate::application::ports::ConfigFiles;
use crate::application::state::InstallOutcome;
use crate::domain::dota_install::DotaInstall;
use crate::domain::gsi_config::{self, ConfigMatch, GsiEndpoint};

/// Operação de arquivo que falhou durante a instalação, com a causa original.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InstallError {
    #[error("não foi possível ler {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("não foi possível criar a pasta {}", dir.display())]
    CreateDir {
        dir: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("não foi possível fazer o backup de {}", path.display())]
    Backup {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("não foi possível gravar {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
}

impl InstallError {
    /// Arquivo ou pasta em que a operação falhou.
    pub fn target(&self) -> &Path {
        match self {
            InstallError::Read { path, .. }
            | InstallError::Backup { path, .. }
            | InstallError::Write { path, .. } => path,
            InstallError::CreateDir { dir, .. } => dir,
        }
    }

    fn io_error(&self) -> &io::Error {
        match self {
            InstallError::Read { source, .. }
            | InstallError::CreateDir { source, .. }
            | InstallError::Backup { source, .. }
            | InstallError::Write { source, .. } => source,
        }
    }
}

impl ToDiagnostic for InstallError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Falha ao instalar o arquivo GSI");
        let target = self.target().display().to_string();
        if self.io_error().kind() != io::ErrorKind::PermissionDenied {
            return Diagnostic::unclassified(
                title,
                format!("Falha ao alterar {target}."),
                self,
                redactor,
            );
        }
        Diagnostic {
            code: DiagnosticCode::GsiConfigInstallPermissionDenied,
            title,
            cause: UiText::with("Sem permissão para alterar {}.", [target]),
            resolution: UiText::new("Corrija as permissões da pasta game/dota/cfg do Dota."),
            technical_details: Some(technical_details(self, redactor)),
        }
    }
}

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
    install_into(files, &install, endpoint, overwrite).unwrap_or_else(InstallOutcome::Failed)
}

fn install_into(
    files: &dyn ConfigFiles,
    install: &DotaInstall,
    endpoint: &GsiEndpoint,
    overwrite: bool,
) -> Result<InstallOutcome, InstallError> {
    let path = install.gsi_config_path();
    let contents = gsi_config::render(endpoint);
    let write_error = |source| InstallError::Write {
        path: path.clone(),
        source: Arc::new(source),
    };

    let existing = files.read(&path).map_err(|source| InstallError::Read {
        path: path.clone(),
        source: Arc::new(source),
    })?;
    let Some(existing) = existing else {
        let dir = install.gsi_dir();
        files
            .ensure_dir(&dir)
            .map_err(|source| InstallError::CreateDir {
                dir: dir.clone(),
                source: Arc::new(source),
            })?;
        files.write(&path, &contents).map_err(write_error)?;
        return Ok(InstallOutcome::Created(path));
    };
    if gsi_config::inspect(&existing, endpoint) == ConfigMatch::Matches {
        return Ok(InstallOutcome::Unchanged(path));
    }
    if !overwrite {
        return Ok(InstallOutcome::NeedsConfirmation(path));
    }
    let backup = files.backup(&path).map_err(|source| InstallError::Backup {
        path: path.clone(),
        source: Arc::new(source),
    })?;
    files.write(&path, &contents).map_err(write_error)?;
    Ok(InstallOutcome::Replaced { path, backup })
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/install_gsi_config_tests.rs"]
mod tests;
