//! Testes unitários de `src/application/use_cases/install_gsi_config.rs`.

use super::*;
use crate::test_support::MemoryFiles;

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

/// Arquivos em memória cuja gravação sempre falha com o tipo de erro informado.
struct ReadOnlyFiles {
    inner: MemoryFiles,
    write_error: io::ErrorKind,
}

impl ConfigFiles for ReadOnlyFiles {
    fn read(&self, path: &Path) -> io::Result<Option<String>> {
        self.inner.read(path)
    }
    fn ensure_dir(&self, dir: &Path) -> io::Result<()> {
        self.inner.ensure_dir(dir)
    }
    fn write(&self, _path: &Path, _contents: &str) -> io::Result<()> {
        Err(self.write_error.into())
    }
    fn backup(&self, path: &Path) -> io::Result<PathBuf> {
        self.inner.backup(path)
    }
}

#[test]
fn creates_config_when_none_exists() {
    let files = MemoryFiles::default();
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);

    let outcome = install_gsi_config(&files, library_root, &endpoint(), false);

    assert!(
        matches!(&outcome, InstallOutcome::Created(path) if *path == config_path),
        "{outcome:?}"
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

    let outcome = install_gsi_config(&files, library_root, &endpoint(), false);

    assert!(
        matches!(&outcome, InstallOutcome::Unchanged(path) if *path == config_path),
        "{outcome:?}"
    );
}

#[test]
fn asks_confirmation_and_keeps_file_when_existing_config_differs() {
    let files = MemoryFiles::default();
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);
    files.put(&config_path, OTHER_APP_CONFIG);

    let outcome = install_gsi_config(&files, library_root, &endpoint(), false);

    assert!(
        matches!(&outcome, InstallOutcome::NeedsConfirmation(path) if *path == config_path),
        "{outcome:?}"
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
    let InstallOutcome::Replaced { path, backup } = outcome else {
        panic!("esperava Replaced, veio {outcome:?}");
    };
    assert_eq!(path, config_path);
    assert_eq!(files.get(&backup).as_deref(), Some(OTHER_APP_CONFIG));
    assert_eq!(
        files.get(&config_path),
        Some(gsi_config::render(&endpoint()))
    );
}

#[test]
fn fails_with_write_error_and_keeps_previous_file_when_write_is_denied() {
    let files = ReadOnlyFiles {
        inner: MemoryFiles::default(),
        write_error: io::ErrorKind::PermissionDenied,
    };
    let library_root = Path::new("/lib");
    let config_path = expected_config_path(library_root);
    files.inner.put(&config_path, OTHER_APP_CONFIG);

    let outcome = install_gsi_config(&files, library_root, &endpoint(), true);
    let InstallOutcome::Failed(InstallError::Write { path, source }) = &outcome else {
        panic!("esperava falha de gravação, veio {outcome:?}");
    };
    assert_eq!(path, &config_path);
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        files.inner.get(&config_path).as_deref(),
        Some(OTHER_APP_CONFIG)
    );
}

#[test]
fn permission_denied_is_classified_with_target_path() {
    let install_error = InstallError::Write {
        path: PathBuf::from("/lib/cfg/dota.cfg"),
        source: Arc::new(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "falha simulada",
        )),
    };
    let diagnostic = install_error.to_diagnostic(&Redactor::default());
    assert_eq!(
        diagnostic.code,
        DiagnosticCode::GsiConfigInstallPermissionDenied
    );
    assert_eq!(
        diagnostic.cause,
        UiText::with("Sem permissão para alterar {}.", ["/lib/cfg/dota.cfg"])
    );
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível gravar /lib/cfg/dota.cfg: falha simulada")
    );
}

#[test]
fn unknown_install_failure_is_unclassified_and_keeps_original_error() {
    let install_error = InstallError::CreateDir {
        dir: PathBuf::from("/lib/cfg"),
        source: Arc::new(io::Error::new(io::ErrorKind::StorageFull, "disco cheio")),
    };
    let diagnostic = install_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::Unclassified);
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("não foi possível criar a pasta /lib/cfg: disco cheio")
    );
}
