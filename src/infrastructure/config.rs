//! Configuração local do aplicativo (`~/.config/dota-companion/config.toml`).
//!
//! O token do GSI é gerado na primeira execução e salvo com permissão 0600.
//! Variáveis de ambiente (ou um `.env` local) sobrescrevem o arquivo.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Porta padrão do servidor GSI local.
pub const DEFAULT_GSI_PORT: u16 = 53000;
const TOKEN_BYTES: usize = 32;

/// Conteúdo do `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub gsi_port: u16,
    pub gsi_token: String,
    /// Bibliotecas Steam fora dos caminhos padrão.
    pub extra_libraries: Vec<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gsi_port: DEFAULT_GSI_PORT,
            gsi_token: String::new(),
            extra_libraries: vec![PathBuf::from("/mnt/jogos/SteamLibrary")],
        }
    }
}

/// Caminho do `config.toml` no diretório de configuração do usuário (XDG).
pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "dota-companion")
        .map(|project_dirs| project_dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("dota-companion.toml"))
}

/// Carrega o arquivo, gera o token se faltar e aplica as variáveis de ambiente
/// `DOTA_COMPANION_GSI_PORT` e `DOTA_COMPANION_EXTRA_LIBRARIES` (separadas por `:`).
pub fn load_or_init(path: &Path) -> io::Result<AppConfig> {
    let mut config = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .map_err(|toml_error| io::Error::new(io::ErrorKind::InvalidData, toml_error))?,
        Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => AppConfig::default(),
        Err(io_error) => return Err(io_error),
    };
    if config.gsi_token.is_empty() {
        config.gsi_token = generate_token()?;
        save(path, &config)?;
    }

    if let Some(port) = std::env::var("DOTA_COMPANION_GSI_PORT")
        .ok()
        .and_then(|port_text| port_text.parse().ok())
    {
        config.gsi_port = port;
    }
    if let Ok(extra_paths) = std::env::var("DOTA_COMPANION_EXTRA_LIBRARIES") {
        config.extra_libraries.extend(
            std::env::split_paths(&extra_paths).filter(|library| !library.as_os_str().is_empty()),
        );
    }
    Ok(config)
}

/// Grava a configuração com permissão 0600.
pub fn save(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(config_dir) = path.parent() {
        fs::create_dir_all(config_dir)?;
    }
    let text = toml::to_string_pretty(config).map_err(io::Error::other)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())
}

fn generate_token() -> io::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
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
}
