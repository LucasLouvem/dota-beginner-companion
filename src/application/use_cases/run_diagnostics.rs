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
mod tests {
    use super::*;
    use crate::application::ports::LocatorResult;
    use crate::application::use_cases::test_support::MemoryFiles;
    use crate::domain::dota_install::DotaInstall;
    use crate::domain::gsi_config::ConfigMatch;
    use std::io;
    use std::path::{Path, PathBuf};

    struct SingleInstallLocator;
    impl DotaLocator for SingleInstallLocator {
        fn locate(&self) -> LocatorResult {
            let library = PathBuf::from("/lib");
            LocatorResult {
                searched_libraries: vec![library.clone()],
                installs: vec![DotaInstall {
                    game_root: DotaInstall::game_root_for(&library),
                    library,
                    native_binary: true,
                    proton_prefix: false,
                }],
            }
        }
    }

    struct FixedProcessProbe(bool);
    impl ProcessProbe for FixedProcessProbe {
        fn dota_running(&self) -> bool {
            self.0
        }
    }

    /// Arquivos que sempre falham na leitura, como uma pasta sem permissão.
    struct UnreadableFiles;
    impl ConfigFiles for UnreadableFiles {
        fn read(&self, _path: &Path) -> io::Result<Option<String>> {
            Err(io::ErrorKind::PermissionDenied.into())
        }
        fn ensure_dir(&self, _dir: &Path) -> io::Result<()> {
            unreachable!("o diagnóstico só lê arquivos")
        }
        fn write(&self, _path: &Path, _contents: &str) -> io::Result<()> {
            unreachable!("o diagnóstico só lê arquivos")
        }
        fn backup(&self, _path: &Path) -> io::Result<PathBuf> {
            unreachable!("o diagnóstico só lê arquivos")
        }
    }

    fn endpoint() -> GsiEndpoint {
        GsiEndpoint {
            port: 53000,
            token: "token".into(),
        }
    }

    #[test]
    fn reports_missing_config_and_closed_dota() {
        let report = run_diagnostics(
            &SingleInstallLocator,
            &FixedProcessProbe(false),
            &MemoryFiles::default(),
            &endpoint(),
        );
        assert_eq!(report.installs[0].config, GsiConfigState::Missing);
        assert!(!report.dota_process_running);
    }

    #[test]
    fn reports_matching_config_and_running_dota() {
        let files = MemoryFiles::default();
        let config_path = DotaInstall::game_root_for(Path::new("/lib"))
            .join("game/dota/cfg/gamestate_integration")
            .join(gsi_config::CONFIG_FILE_NAME);
        files.put(&config_path, &gsi_config::render(&endpoint()));

        let report = run_diagnostics(
            &SingleInstallLocator,
            &FixedProcessProbe(true),
            &files,
            &endpoint(),
        );
        assert_eq!(
            report.installs[0].config,
            GsiConfigState::Present(ConfigMatch::Matches)
        );
        assert!(report.dota_process_running);
    }

    #[test]
    fn reports_unreadable_config_when_read_fails() {
        let report = run_diagnostics(
            &SingleInstallLocator,
            &FixedProcessProbe(false),
            &UnreadableFiles,
            &endpoint(),
        );
        assert!(matches!(
            report.installs[0].config,
            GsiConfigState::Unreadable(_)
        ));
    }
}
