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
mod tests {
    use super::*;
    use crate::application::use_cases::test_support::MemoryFiles;
    use std::path::PathBuf;

    fn endpoint() -> GsiEndpoint {
        GsiEndpoint {
            port: 53000,
            token: "secret".into(),
        }
    }

    fn expected_config_path(library_root: &Path) -> PathBuf {
        DotaInstall::game_root_for(library_root)
            .join("game/dota/cfg/gamestate_integration")
            .join(gsi_config::CONFIG_FILE_NAME)
    }

    const OTHER_APP_CONFIG: &str = "\"old\" { \"uri\" \"http://elsewhere\" }";

    #[test]
    fn creates_config_when_none_exists() {
        let files = MemoryFiles::default();
        let library_root = Path::new("/lib");
        let config_path = expected_config_path(library_root);

        assert_eq!(
            install_gsi_config(&files, library_root, &endpoint(), false),
            InstallOutcome::Created(config_path.clone())
        );
        assert_eq!(
            files.get(&config_path),
            Some(gsi_config::render(&endpoint()))
        );
    }

    #[test]
    fn reports_unchanged_when_config_already_matches() {
        let files = MemoryFiles::default();
        let library_root = Path::new("/lib");
        let config_path = expected_config_path(library_root);
        files.put(&config_path, &gsi_config::render(&endpoint()));

        assert_eq!(
            install_gsi_config(&files, library_root, &endpoint(), false),
            InstallOutcome::Unchanged(config_path)
        );
    }

    #[test]
    fn asks_confirmation_and_keeps_file_when_existing_config_differs() {
        let files = MemoryFiles::default();
        let library_root = Path::new("/lib");
        let config_path = expected_config_path(library_root);
        files.put(&config_path, OTHER_APP_CONFIG);

        assert_eq!(
            install_gsi_config(&files, library_root, &endpoint(), false),
            InstallOutcome::NeedsConfirmation(config_path.clone())
        );
        assert_eq!(files.get(&config_path).as_deref(), Some(OTHER_APP_CONFIG));
    }

    #[test]
    fn backs_up_and_replaces_when_overwrite_is_confirmed() {
        let files = MemoryFiles::default();
        let library_root = Path::new("/lib");
        let config_path = expected_config_path(library_root);
        files.put(&config_path, OTHER_APP_CONFIG);

        let outcome = install_gsi_config(&files, library_root, &endpoint(), true);
        let InstallOutcome::Replaced { backup, .. } = outcome else {
            panic!("esperava Replaced, veio {outcome:?}");
        };
        assert_eq!(files.get(&backup).as_deref(), Some(OTHER_APP_CONFIG));
        assert_eq!(
            files.get(&config_path),
            Some(gsi_config::render(&endpoint()))
        );
    }
}
