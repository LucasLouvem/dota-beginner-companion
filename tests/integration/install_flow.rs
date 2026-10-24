//! Comando "Instalar configuração GSI" passando pelo núcleo, pelo caso de uso
//! e pelos arquivos reais, até o resultado e o novo diagnóstico no `AppSnapshot`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use dota_companion::application::diagnostics::GsiConfigState;
use dota_companion::application::error::{DiagnosticCode, Redactor, ToDiagnostic};
use dota_companion::application::event_bus::UiCommand;
use dota_companion::application::state::{AppSnapshot, InstallOutcome};
use dota_companion::domain::dota_install::DotaInstall;
use dota_companion::domain::gsi_config::{CONFIG_FILE_NAME, GsiEndpoint};

use super::harness::RunningCore;

fn endpoint() -> GsiEndpoint {
    GsiEndpoint {
        port: 53000,
        token: "integration-token".into(),
    }
}

fn config_path(library: &Path) -> std::path::PathBuf {
    DotaInstall::game_root_for(library)
        .join("game/dota/cfg/gamestate_integration")
        .join(CONFIG_FILE_NAME)
}

async fn install(core: &mut RunningCore, library: &Path, overwrite: bool) -> AppSnapshot {
    let attempt = core.state.borrow().install_attempt;
    core.commands
        .send(UiCommand::InstallGsiConfig {
            install_root: library.to_path_buf(),
            overwrite,
        })
        .unwrap();
    core.wait_for(|snapshot| snapshot.install_attempt == attempt + 1)
        .await
}

#[tokio::test]
async fn install_creates_config_and_next_diagnostic_reports_it_as_matching() {
    let temp_dir = tempfile::tempdir().unwrap();
    let library = temp_dir.path();
    let mut core = RunningCore::start(endpoint(), Some(library));

    let snapshot = install(&mut core, library, false).await;

    assert!(
        matches!(&snapshot.last_install, Some(InstallOutcome::Created(path)) if *path == config_path(library)),
        "{:?}",
        snapshot.last_install
    );
    let report = snapshot
        .diagnostics
        .expect("a instalação roda um novo diagnóstico");
    assert!(
        matches!(report.installs[0].config, GsiConfigState::Matches),
        "{:?}",
        report.installs[0].config
    );
    let contents = fs::read_to_string(config_path(library)).unwrap();
    assert!(contents.contains("\"token\" \"integration-token\""));
}

#[tokio::test]
async fn different_config_needs_confirmation_and_is_replaced_with_backup_after_it() {
    let temp_dir = tempfile::tempdir().unwrap();
    let library = temp_dir.path();
    let path = config_path(library);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "\"other\" { \"uri\" \"http://elsewhere\" }").unwrap();
    let mut core = RunningCore::start(endpoint(), Some(library));

    let snapshot = install(&mut core, library, false).await;
    assert!(
        matches!(
            &snapshot.last_install,
            Some(InstallOutcome::NeedsConfirmation(_))
        ),
        "{:?}",
        snapshot.last_install
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "\"other\" { \"uri\" \"http://elsewhere\" }"
    );

    let snapshot = install(&mut core, library, true).await;
    let Some(InstallOutcome::Replaced { backup, .. }) = &snapshot.last_install else {
        panic!("esperava substituição: {:?}", snapshot.last_install);
    };
    assert_eq!(
        fs::read_to_string(backup).unwrap(),
        "\"other\" { \"uri\" \"http://elsewhere\" }"
    );
    assert!(matches!(
        snapshot.diagnostics.unwrap().installs[0].config,
        GsiConfigState::Matches
    ));
}

#[tokio::test]
async fn install_into_read_only_library_is_shown_as_permission_denied() {
    let temp_dir = tempfile::tempdir().unwrap();
    let library = temp_dir.path();
    fs::set_permissions(library, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::write(library.join("probe"), "").is_ok() {
        // Executando como root: permissões não se aplicam.
        return;
    }
    let mut core = RunningCore::start(endpoint(), Some(library));

    let snapshot = install(&mut core, library, false).await;
    fs::set_permissions(library, fs::Permissions::from_mode(0o700)).unwrap();

    let Some(InstallOutcome::Failed(install_error)) = &snapshot.last_install else {
        panic!("esperava falha: {:?}", snapshot.last_install);
    };
    assert_eq!(
        install_error.to_diagnostic(&Redactor::default()).code,
        DiagnosticCode::GsiConfigInstallPermissionDenied
    );
}
