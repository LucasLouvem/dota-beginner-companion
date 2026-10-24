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
                Err(e) => GsiConfigState::Unreadable(e.to_string()),
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
    use std::path::PathBuf;

    struct OneInstall;
    impl DotaLocator for OneInstall {
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

    struct Running(bool);
    impl ProcessProbe for Running {
        fn dota_running(&self) -> bool {
            self.0
        }
    }

    #[test]
    fn reports_missing_and_matching_config() {
        let endpoint = GsiEndpoint {
            port: 53000,
            token: "t".into(),
        };
        let files = MemoryFiles::default();

        let report = run_diagnostics(&OneInstall, &Running(false), &files, &endpoint);
        assert_eq!(report.installs[0].config, GsiConfigState::Missing);
        assert!(!report.dota_process_running);

        let path = report.installs[0].config_path.clone();
        files.put(&path, &gsi_config::render(&endpoint));
        let report = run_diagnostics(&OneInstall, &Running(true), &files, &endpoint);
        assert_eq!(
            report.installs[0].config,
            GsiConfigState::Present(ConfigMatch::Matches)
        );
        assert!(report.dota_process_running);
    }
}
