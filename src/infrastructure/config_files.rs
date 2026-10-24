//! Adaptador de arquivos para ler, gravar e fazer backup dos `.cfg` do GSI.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::ConfigFiles;

/// Implementação de `ConfigFiles` no sistema de arquivos local.
pub struct LocalConfigFiles;

impl ConfigFiles for LocalConfigFiles {
    fn read(&self, path: &Path) -> io::Result<Option<String>> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(io_error) => Err(io_error),
        }
    }

    fn ensure_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)
    }

    /// Escreve em arquivo temporário e renomeia, para nunca deixar um `.cfg`
    /// pela metade.
    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        let partial_path = path.with_extension("cfg.tmp");
        fs::write(&partial_path, contents)?;
        fs::rename(&partial_path, path).inspect_err(|_| {
            let _ = fs::remove_file(&partial_path);
        })
    }

    fn backup(&self, path: &Path) -> io::Result<PathBuf> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut backup = path.with_file_name(format!("{file_name}.bak-{timestamp}"));
        let mut attempt = 1;
        while backup.exists() {
            backup = path.with_file_name(format!("{file_name}.bak-{timestamp}-{attempt}"));
            attempt += 1;
        }
        fs::copy(path, &backup)?;
        Ok(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::state::InstallOutcome;
    use crate::application::use_cases::install_gsi_config::install_gsi_config;
    use crate::domain::gsi_config::GsiEndpoint;
    use std::os::unix::fs::PermissionsExt;

    fn endpoint() -> GsiEndpoint {
        GsiEndpoint {
            port: 53000,
            token: "secret".into(),
        }
    }

    fn temporary_files_in(dir: &Path) -> usize {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count()
    }

    #[test]
    fn creates_config_in_real_directory_without_leftover_temporary_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let outcome = install_gsi_config(&LocalConfigFiles, temp_dir.path(), &endpoint(), false);
        let InstallOutcome::Created(config_path) = outcome else {
            panic!("esperava Created, veio {outcome:?}");
        };
        assert!(config_path.is_file());
        assert_eq!(temporary_files_in(config_path.parent().unwrap()), 0);
    }

    #[test]
    fn keeps_previous_content_in_backup_when_replacing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let InstallOutcome::Created(config_path) =
            install_gsi_config(&LocalConfigFiles, temp_dir.path(), &endpoint(), false)
        else {
            panic!("instalação inicial falhou");
        };
        fs::write(&config_path, "\"other\" {}").unwrap();

        let outcome = install_gsi_config(&LocalConfigFiles, temp_dir.path(), &endpoint(), true);
        let InstallOutcome::Replaced { backup, .. } = outcome else {
            panic!("esperava Replaced, veio {outcome:?}");
        };
        assert_eq!(fs::read_to_string(backup).unwrap(), "\"other\" {}");
    }

    #[test]
    fn fails_with_path_when_directory_is_not_writable() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
        if fs::write(temp_dir.path().join("probe"), "").is_ok() {
            // Executando como root: permissões não se aplicam.
            return;
        }
        let outcome = install_gsi_config(&LocalConfigFiles, temp_dir.path(), &endpoint(), false);
        assert!(
            matches!(outcome, InstallOutcome::Failed { .. }),
            "{outcome:?}"
        );
        fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
}
