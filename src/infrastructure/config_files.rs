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
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn ensure_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)
    }

    /// Escreve em arquivo temporário e renomeia, para nunca deixar um `.cfg`
    /// pela metade.
    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        let tmp = path.with_extension("cfg.tmp");
        fs::write(&tmp, contents)?;
        fs::rename(&tmp, path).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }

    fn backup(&self, path: &Path) -> io::Result<PathBuf> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut backup = path.with_file_name(format!("{name}.bak-{stamp}"));
        let mut n = 1;
        while backup.exists() {
            backup = path.with_file_name(format!("{name}.bak-{stamp}-{n}"));
            n += 1;
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

    #[test]
    fn installs_into_real_directory_and_backs_up() {
        let tmp = tempfile::tempdir().unwrap();
        let outcome = install_gsi_config(&LocalConfigFiles, tmp.path(), &endpoint(), false);
        let InstallOutcome::Created(path) = outcome else {
            panic!("esperava Created, veio {outcome:?}");
        };
        assert!(path.is_file());

        fs::write(&path, "\"other\" {}").unwrap();
        let outcome = install_gsi_config(&LocalConfigFiles, tmp.path(), &endpoint(), true);
        let InstallOutcome::Replaced { backup, .. } = outcome else {
            panic!("esperava Replaced, veio {outcome:?}");
        };
        assert_eq!(fs::read_to_string(backup).unwrap(), "\"other\" {}");
        // Sem arquivos temporários sobrando.
        let leftovers = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn reports_permission_errors() {
        let tmp = tempfile::tempdir().unwrap();
        fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o500)).unwrap();
        if fs::write(tmp.path().join("probe"), "").is_ok() {
            // Executando como root: permissões não se aplicam.
            return;
        }
        let outcome = install_gsi_config(&LocalConfigFiles, tmp.path(), &endpoint(), false);
        assert!(
            matches!(outcome, InstallOutcome::Failed { .. }),
            "{outcome:?}"
        );
        fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
}
