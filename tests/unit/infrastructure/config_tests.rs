//! Testes unitários de `src/infrastructure/config.rs`.

use super::*;
use std::os::unix::fs::PermissionsExt;

/// Ambiente vazio, para os testes não dependerem das variáveis da máquina.
fn no_environment(_name: &str) -> Option<String> {
    None
}

const SIMULATED_SECRET: &str = "segredo-simulado-123";

fn write_env_file(contents: &str) -> (tempfile::TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let env_path = temp_dir.path().join(ENV_FILE_NAME);
    fs::write(&env_path, contents).unwrap();
    (temp_dir, env_path)
}

#[test]
fn missing_env_file_is_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let env_file = read_env_file(&temp_dir.path().join(ENV_FILE_NAME)).unwrap();
    assert_eq!(env_file, EnvFile::default());
}

#[test]
fn reads_env_values_keeping_the_first_occurrence() {
    let (_temp_dir, env_path) = write_env_file(
        "# comentário\n\
         DOTA_COMPANION_GSI_PORT=53000\n\
         DOTA_COMPANION_EXTRA_LIBRARIES=/mnt/jogos/SteamLibrary\n\
         RUST_LOG=info\n\
         DOTA_COMPANION_GSI_LOG=\n\
         DOTA_COMPANION_GSI_PORT=4000\n",
    );

    let env_file = read_env_file(&env_path).unwrap();

    assert_eq!(
        env_file.get("DOTA_COMPANION_GSI_PORT").as_deref(),
        Some("53000")
    );
    assert_eq!(
        env_file.get("DOTA_COMPANION_EXTRA_LIBRARIES").as_deref(),
        Some("/mnt/jogos/SteamLibrary")
    );
    assert_eq!(env_file.get("RUST_LOG").as_deref(), Some("info"));
    assert_eq!(env_file.get("DOTA_COMPANION_GSI_LOG").as_deref(), Some(""));
    assert_eq!(env_file.get("MISSING"), None);
}

#[test]
fn ignores_byte_order_mark_at_the_start_of_env_file() {
    let (_temp_dir, env_path) = write_env_file("\u{feff}RUST_LOG=debug\n");
    let env_file = read_env_file(&env_path).unwrap();
    assert_eq!(env_file.get("RUST_LOG").as_deref(), Some("debug"));
}

#[test]
fn fails_with_position_and_without_line_contents_when_env_syntax_is_invalid() {
    let (_temp_dir, env_path) =
        write_env_file(&format!("RUST_LOG=info\nTOKEN {SIMULATED_SECRET}\n"));

    let env_error = read_env_file(&env_path).unwrap_err();
    let EnvFileError::Syntax { path, position, .. } = &env_error else {
        panic!("esperava erro de sintaxe: {env_error:?}");
    };
    assert_eq!(path, &env_path);
    // Depois de `TOKEN ` o dotenvy espera `=` e acusa o caractere seguinte.
    assert_eq!(*position, Some(TextPosition { line: 2, column: 7 }));

    let diagnostic = env_error.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::EnvFileInvalid);
    assert_eq!(
        diagnostic.technical_details,
        Some(format!(
            "{} tem uma linha com sintaxe inválida na linha 2, coluna 7",
            env_path.display()
        ))
    );
    assert!(!format!("{diagnostic:?}").contains(SIMULATED_SECRET));
}

#[test]
fn fails_with_unclassified_read_error_when_env_path_is_a_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let env_path = temp_dir.path().join(ENV_FILE_NAME);
    fs::create_dir(&env_path).unwrap();

    let env_error = read_env_file(&env_path).unwrap_err();
    let EnvFileError::Read { path, source } = &env_error else {
        panic!("esperava erro de leitura: {env_error:?}");
    };
    assert_eq!(path, &env_path);
    assert_eq!(source.kind(), io::ErrorKind::IsADirectory);
    assert_eq!(
        env_error.to_diagnostic(&Redactor::default()).code,
        DiagnosticCode::Unclassified
    );
}

#[test]
fn fails_with_permission_denied_when_env_file_is_unreadable() {
    let (_temp_dir, env_path) = write_env_file("RUST_LOG=info\n");
    fs::set_permissions(&env_path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&env_path).is_ok() {
        // Executando como root: permissões não se aplicam.
        return;
    }

    let env_error = read_env_file(&env_path).unwrap_err();
    assert!(
        matches!(&env_error, EnvFileError::Read { source, .. } if source.kind() == io::ErrorKind::PermissionDenied),
        "{env_error:?}"
    );
    assert_eq!(
        env_error.to_diagnostic(&Redactor::default()).code,
        DiagnosticCode::EnvFilePermissionDenied
    );
}

#[test]
fn applies_values_from_the_given_lookup_when_loading_config() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");

    let config = load_or_init(&config_file, |name| {
        (name == "DOTA_COMPANION_GSI_PORT").then(|| "4000".to_owned())
    })
    .unwrap();

    assert_eq!(config.gsi_port, 4000);
}

#[test]
fn first_run_creates_config_with_token_and_private_permissions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("sub/config.toml");

    let config = load_or_init(&config_file, no_environment).unwrap();
    assert_eq!(config.gsi_token.len(), 64);
    assert!(
        config
            .gsi_token
            .chars()
            .all(|character| character.is_ascii_hexdigit()),
        "o token deve ser hexadecimal"
    );
    let saved: AppConfig = toml::from_str(&fs::read_to_string(&config_file).unwrap()).unwrap();
    assert_eq!(saved.gsi_token, config.gsi_token);
    assert_eq!(
        fs::metadata(&config_file).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn later_runs_keep_the_same_token() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");

    let first_run = load_or_init(&config_file, no_environment).unwrap();
    let second_run = load_or_init(&config_file, no_environment).unwrap();
    assert_eq!(first_run.gsi_token, second_run.gsi_token);
}

#[test]
fn fails_with_path_and_position_when_config_file_has_invalid_values() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");
    fs::write(
        &config_file,
        "extra_libraries = []\ngsi_port = \"not a number\"",
    )
    .unwrap();

    let result = load_or_init(&config_file, no_environment);
    let Err(ConfigError::Parse { path, position, .. }) = &result else {
        panic!("esperava erro de conteúdo: {result:?}");
    };
    assert_eq!(path, &config_file);
    assert_eq!(
        *position,
        Some(TextPosition {
            line: 2,
            column: 12
        })
    );
}

#[test]
fn invalid_config_diagnostic_shows_position_without_file_contents() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");
    fs::write(&config_file, "gsi_token = 987654321").unwrap();

    let config_error = load_or_init(&config_file, no_environment).unwrap_err();
    let diagnostic = config_error.to_diagnostic(&Redactor::default());

    assert_eq!(diagnostic.code, DiagnosticCode::AppConfigInvalid);
    assert_eq!(
        diagnostic.technical_details,
        Some(format!(
            "{} tem conteúdo inválido na linha 1, coluna 13",
            config_file.display()
        ))
    );
}

#[test]
fn fails_with_read_error_classified_as_permission_denied_when_file_is_unreadable() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");
    fs::write(&config_file, "").unwrap();
    fs::set_permissions(&config_file, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&config_file).is_ok() {
        // Executando como root: permissões não se aplicam.
        return;
    }

    let config_error = load_or_init(&config_file, no_environment).unwrap_err();
    let ConfigError::Read { path, source } = &config_error else {
        panic!("esperava erro de leitura: {config_error:?}");
    };
    assert_eq!(path, &config_file);
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        config_error.to_diagnostic(&Redactor::default()).code,
        DiagnosticCode::AppConfigPermissionDenied
    );
}

#[test]
fn text_position_counts_lines_and_characters_from_one() {
    let text = "a = 1\nção = 2";
    assert_eq!(
        TextPosition::of(text, 0),
        TextPosition { line: 1, column: 1 }
    );
    assert_eq!(
        TextPosition::of(text, text.find('=').unwrap()),
        TextPosition { line: 1, column: 3 }
    );
    let second_equals = text.rfind('=').unwrap();
    assert_eq!(
        TextPosition::of(text, second_equals),
        TextPosition { line: 2, column: 5 }
    );
}

fn environment<'a>(variables: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        variables
            .iter()
            .find(|(variable, _)| *variable == name)
            .map(|(_, value)| value.to_string())
    }
}

#[test]
fn environment_overrides_port_and_adds_libraries_and_log_path() {
    let mut config = AppConfig::default();
    apply_environment(
        &mut config,
        environment(&[
            ("DOTA_COMPANION_GSI_PORT", "4000"),
            ("DOTA_COMPANION_EXTRA_LIBRARIES", "/games/a:/games/b"),
            ("DOTA_COMPANION_GSI_LOG", "logs/gsi.jsonl"),
        ]),
    );
    assert_eq!(config.gsi_port, 4000);
    assert!(
        config
            .extra_libraries
            .ends_with(&[PathBuf::from("/games/a"), PathBuf::from("/games/b")])
    );
    assert_eq!(config.gsi_log, Some(PathBuf::from("logs/gsi.jsonl")));
}

#[test]
fn keeps_defaults_when_variables_are_missing_or_invalid() {
    let mut config = AppConfig::default();
    apply_environment(
        &mut config,
        environment(&[
            ("DOTA_COMPANION_GSI_PORT", "not a port"),
            ("DOTA_COMPANION_GSI_LOG", ""),
        ]),
    );
    assert_eq!(config, AppConfig::default());
}

#[test]
fn log_path_is_never_saved_to_config_file() {
    let config = AppConfig {
        gsi_log: Some(PathBuf::from("logs/gsi.jsonl")),
        ..AppConfig::default()
    };
    assert!(!toml::to_string(&config).unwrap().contains("gsi_log"));
}
