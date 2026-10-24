//! Testes unitários de `src/infrastructure/config.rs`.

use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn first_run_creates_config_with_token_and_private_permissions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("sub/config.toml");

    let config = load_or_init(&config_file).unwrap();
    assert_eq!(config.gsi_token.len(), TOKEN_BYTES * 2);
    assert_eq!(
        fs::metadata(&config_file).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn later_runs_keep_the_same_token() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");

    let first_run = load_or_init(&config_file).unwrap();
    let second_run = load_or_init(&config_file).unwrap();
    assert_eq!(first_run.gsi_token, second_run.gsi_token);
}

#[test]
fn fails_when_config_file_has_invalid_values() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_file = temp_dir.path().join("config.toml");
    fs::write(&config_file, "gsi_port = \"not a number\"").unwrap();
    assert!(load_or_init(&config_file).is_err());
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
