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
    let failed = |e: std::io::Error| InstallOutcome::Failed {
        path: path.clone(),
        error: e.to_string(),
    };

    let contents = gsi_config::render(endpoint);
    match files.read(&path) {
        Err(e) => failed(e),
        Ok(None) => match files
            .ensure_dir(&install.gsi_dir())
            .and_then(|()| files.write(&path, &contents))
        {
            Ok(()) => InstallOutcome::Created(path.clone()),
            Err(e) => failed(e),
        },
        Ok(Some(existing)) => {
            if gsi_config::inspect(&existing, endpoint) == ConfigMatch::Matches {
                return InstallOutcome::Unchanged(path.clone());
            }
            if !overwrite {
                return InstallOutcome::NeedsConfirmation(path.clone());
            }
            match files.backup(&path) {
                Err(e) => failed(e),
                Ok(backup) => match files.write(&path, &contents) {
                    Ok(()) => InstallOutcome::Replaced {
                        path: path.clone(),
                        backup,
                    },
                    Err(e) => failed(e),
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

    fn cfg_path(root: &Path) -> PathBuf {
        DotaInstall::game_root_for(root)
            .join("game/dota/cfg/gamestate_integration")
            .join(gsi_config::CONFIG_FILE_NAME)
    }

    #[test]
    fn creates_then_reports_unchanged() {
        let files = MemoryFiles::default();
        let root = Path::new("/lib");
        let path = cfg_path(root);

        assert_eq!(
            install_gsi_config(&files, root, &endpoint(), false),
            InstallOutcome::Created(path.clone())
        );
        assert_eq!(files.get(&path), Some(gsi_config::render(&endpoint())));
        assert_eq!(
            install_gsi_config(&files, root, &endpoint(), false),
            InstallOutcome::Unchanged(path)
        );
    }

    #[test]
    fn asks_before_replacing_and_backs_up() {
        let files = MemoryFiles::default();
        let root = Path::new("/lib");
        let path = cfg_path(root);
        files.put(&path, "\"old\" { \"uri\" \"http://elsewhere\" }");

        assert_eq!(
            install_gsi_config(&files, root, &endpoint(), false),
            InstallOutcome::NeedsConfirmation(path.clone())
        );
        assert!(files.get(&path).unwrap().contains("elsewhere"));

        let outcome = install_gsi_config(&files, root, &endpoint(), true);
        let InstallOutcome::Replaced { backup, .. } = outcome else {
            panic!("esperava Replaced, veio {outcome:?}");
        };
        assert!(files.get(&backup).unwrap().contains("elsewhere"));
        assert_eq!(files.get(&path), Some(gsi_config::render(&endpoint())));
    }
}
