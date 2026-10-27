//! Testes unitários de `src/application/use_cases/run_diagnostics.rs`.

use super::*;
use crate::application::ports::LocatorResult;
use crate::domain::dota_install::DotaInstall;
use crate::domain::gsi_config::GsiConfigParseError;
use crate::domain::keyvalues::KeyValuesError;
use crate::test_support::MemoryFiles;
use std::io;
use std::path::PathBuf;

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

/// Sonda fixa: o booleano vale para o Dota; a Steam está aberta quando o Dota está.
struct FixedProcessProbe(bool);
impl ProcessProbe for FixedProcessProbe {
    fn dota_running(&self) -> bool {
        self.0
    }
    fn steam_running(&self) -> bool {
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

fn expected_config_path() -> PathBuf {
    DotaInstall::game_root_for(Path::new("/lib"))
        .join("game/dota/cfg/gamestate_integration")
        .join(gsi_config::CONFIG_FILE_NAME)
}

#[test]
fn reports_missing_config_and_closed_dota() {
    let report = run_diagnostics(
        &SingleInstallLocator,
        &FixedProcessProbe(false),
        &MemoryFiles::default(),
        &endpoint(),
    );
    assert!(matches!(report.installs[0].config, GsiConfigState::Missing));
    assert!(!report.dota_process_running);
}

#[test]
fn reports_matching_config_and_running_dota() {
    let files = MemoryFiles::default();
    files.put(&expected_config_path(), &gsi_config::render(&endpoint()));

    let report = run_diagnostics(
        &SingleInstallLocator,
        &FixedProcessProbe(true),
        &files,
        &endpoint(),
    );
    assert!(matches!(report.installs[0].config, GsiConfigState::Matches));
    assert!(report.dota_process_running);
    assert!(report.steam_process_running);
}

#[test]
fn reports_read_failure_with_path_and_original_error_when_read_fails() {
    let report = run_diagnostics(
        &SingleInstallLocator,
        &FixedProcessProbe(false),
        &UnreadableFiles,
        &endpoint(),
    );
    let GsiConfigState::Failed(GsiConfigFileError::Read { path, source }) =
        &report.installs[0].config
    else {
        panic!("esperava falha de leitura: {:?}", report.installs[0].config);
    };
    assert_eq!(path, &expected_config_path());
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn reports_invalid_config_with_parser_cause_when_file_is_not_keyvalues() {
    let files = MemoryFiles::default();
    files.put(&expected_config_path(), "\"broken\" {");

    let report = run_diagnostics(
        &SingleInstallLocator,
        &FixedProcessProbe(false),
        &files,
        &endpoint(),
    );
    let GsiConfigState::Failed(GsiConfigFileError::Invalid { path, source }) =
        &report.installs[0].config
    else {
        panic!("esperava arquivo inválido: {:?}", report.installs[0].config);
    };
    assert_eq!(path, &expected_config_path());
    assert_eq!(
        source,
        &GsiConfigParseError::KeyValues(KeyValuesError::UnclosedSection)
    );
}
