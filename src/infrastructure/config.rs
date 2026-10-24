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
        .map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("dota-companion.toml"))
}

/// Carrega o arquivo, gera o token se faltar e aplica as variáveis de ambiente
/// `DOTA_COMPANION_GSI_PORT` e `DOTA_COMPANION_EXTRA_LIBRARIES` (separadas por `:`).
pub fn load_or_init(path: &Path) -> io::Result<AppConfig> {
    let mut config = match fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => AppConfig::default(),
        Err(e) => return Err(e),
    };
    if config.gsi_token.is_empty() {
        config.gsi_token = generate_token()?;
        save(path, &config)?;
    }

    if let Some(port) = std::env::var("DOTA_COMPANION_GSI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
    {
        config.gsi_port = port;
    }
    if let Ok(extra) = std::env::var("DOTA_COMPANION_EXTRA_LIBRARIES") {
        config
            .extra_libraries
            .extend(std::env::split_paths(&extra).filter(|p| !p.as_os_str().is_empty()));
    }
    Ok(config)
}

/// Grava a configuração com permissão 0600.
pub fn save(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
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
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn generates_token_once_with_private_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub/config.toml");

        let first = load_or_init(&path).unwrap();
        assert_eq!(first.gsi_token.len(), TOKEN_BYTES * 2);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let second = load_or_init(&path).unwrap();
        assert_eq!(first.gsi_token, second.gsi_token);
    }

    #[test]
    fn rejects_corrupt_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        fs::write(&path, "gsi_port = \"x\"").unwrap();
        assert!(load_or_init(&path).is_err());
    }
}
