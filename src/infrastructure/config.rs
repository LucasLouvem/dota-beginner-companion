//! Configuração local do aplicativo (`~/.config/dota-companion/config.toml`).
//!
//! O token do GSI é gerado na primeira execução e salvo com permissão 0600.
//! Variáveis de ambiente e o `.env` do diretório de execução sobrescrevem o
//! arquivo; o ambiente tem prioridade sobre o `.env`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::application::error::{
    Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText, technical_details,
};

/// Porta padrão do servidor GSI local.
pub const DEFAULT_GSI_PORT: u16 = 53000;
const TOKEN_BYTES: usize = 32;

/// Falha ao carregar ou salvar o `config.toml`.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("não foi possível ler {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// O erro do `toml` cita o trecho do arquivo, que pode conter o token; por
    /// isso só a posição aparece nos detalhes exibidos.
    #[error("{} tem conteúdo inválido", path.display())]
    Parse {
        path: PathBuf,
        position: Option<TextPosition>,
        /// Em `Box` porque o erro do `toml` é grande e aumentaria todo `Result`.
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("não foi possível gravar {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("não foi possível converter a configuração para TOML")]
    Serialize(#[source] toml::ser::Error),
    #[error("não foi possível gerar o token do GSI")]
    TokenGeneration(#[source] getrandom::Error),
}

/// Linha e coluna (a partir de 1) de um ponto do arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize,
}

impl TextPosition {
    /// Posição do byte `offset` em `text`.
    pub fn of(text: &str, offset: usize) -> Self {
        let before = &text[..text.floor_char_boundary(offset)];
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        Self {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

/// Detalhes de um erro de conteúdo só com a mensagem própria e a posição,
/// sem a mensagem do parser, que cita o trecho do arquivo.
fn positioned_details(
    error: &dyn std::fmt::Display,
    position: Option<TextPosition>,
    redactor: &Redactor,
) -> String {
    redactor.redact(&match position {
        Some(position) => format!(
            "{error} na linha {}, coluna {}",
            position.line, position.column
        ),
        None => error.to_string(),
    })
}

impl ToDiagnostic for ConfigError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Não foi possível carregar a configuração do aplicativo");
        match self {
            ConfigError::Parse { path, position, .. } => Diagnostic {
                code: DiagnosticCode::AppConfigInvalid,
                title,
                cause: UiText::with("{} tem conteúdo inválido.", [path.display().to_string()]),
                resolution: UiText::new(
                    "Corrija o arquivo ou apague-o para criar um novo; com um novo token, \
                     reinstale o arquivo GSI.",
                ),
                technical_details: Some(positioned_details(self, *position, redactor)),
            },
            ConfigError::Read { path, source } | ConfigError::Write { path, source }
                if source.kind() == io::ErrorKind::PermissionDenied =>
            {
                Diagnostic {
                    code: DiagnosticCode::AppConfigPermissionDenied,
                    title,
                    cause: UiText::with(
                        "Sem permissão para acessar {}.",
                        [path.display().to_string()],
                    ),
                    resolution: UiText::new("Corrija as permissões do arquivo ou da sua pasta."),
                    technical_details: Some(technical_details(self, redactor)),
                }
            }
            _ => Diagnostic::unclassified(
                title,
                "A configuração não pôde ser lida nem criada.",
                self,
                redactor,
            ),
        }
    }
}

/// Nome do arquivo de variáveis lido no diretório de execução.
pub const ENV_FILE_NAME: &str = ".env";

/// Variáveis lidas do `.env`. Nunca são gravadas no ambiente do processo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvFile {
    values: HashMap<String, String>,
}

impl EnvFile {
    /// Valor da variável no `.env`, se houver.
    pub fn get(&self, name: &str) -> Option<String> {
        self.values.get(name).cloned()
    }
}

/// Falha ao ler o `.env`. Um `.env` ausente não é erro.
#[derive(Debug, thiserror::Error)]
pub enum EnvFileError {
    #[error("não foi possível ler {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// A mensagem do dotenvy cita a linha inteira, que pode conter segredos;
    /// por isso só a posição aparece nos detalhes exibidos.
    #[error("{} tem uma linha com sintaxe inválida", path.display())]
    Syntax {
        path: PathBuf,
        position: Option<TextPosition>,
        #[source]
        source: dotenvy::Error,
    },
    /// Falha do dotenvy que não é de sintaxe, como uma variável referenciada
    /// com `${...}` que não é texto válido.
    #[error("{} não pôde ser interpretado", path.display())]
    Load {
        path: PathBuf,
        #[source]
        source: dotenvy::Error,
    },
}

impl ToDiagnostic for EnvFileError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Não foi possível carregar o arquivo .env");
        match self {
            EnvFileError::Syntax { path, position, .. } => Diagnostic {
                code: DiagnosticCode::EnvFileInvalid,
                title,
                cause: UiText::with(
                    "{} tem uma linha com sintaxe inválida.",
                    [path.display().to_string()],
                ),
                resolution: UiText::new(
                    "Corrija a linha indicada ou remova o .env, que é opcional.",
                ),
                technical_details: Some(positioned_details(self, *position, redactor)),
            },
            EnvFileError::Read { path, source }
                if source.kind() == io::ErrorKind::PermissionDenied =>
            {
                Diagnostic {
                    code: DiagnosticCode::EnvFilePermissionDenied,
                    title,
                    cause: UiText::with("Sem permissão para ler {}.", [path.display().to_string()]),
                    resolution: UiText::new(
                        "Corrija a permissão do arquivo ou remova o .env, que é opcional.",
                    ),
                    technical_details: Some(technical_details(self, redactor)),
                }
            }
            _ => Diagnostic::unclassified(title, "O .env não pôde ser lido.", self, redactor),
        }
    }
}

/// Lê as variáveis do `.env` sem alterar o ambiente do processo. Nada é
/// devolvido se alguma linha falhar, para nunca usar um `.env` lido pela
/// metade. Como no dotenvy, vale a primeira ocorrência de cada variável.
pub fn read_env_file(path: &Path) -> Result<EnvFile, EnvFileError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => {
            return Ok(EnvFile::default());
        }
        Err(io_error) => {
            return Err(EnvFileError::Read {
                path: path.to_path_buf(),
                source: io_error,
            });
        }
    };
    // O `Iter::load` do dotenvy remove o BOM; a leitura item a item não.
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(&contents);

    let mut values = HashMap::new();
    for item in dotenvy::from_read_iter(contents.as_bytes()) {
        let (name, value) = item.map_err(|dotenv_error| match &dotenv_error {
            dotenvy::Error::LineParse(line, index) => EnvFileError::Syntax {
                path: path.to_path_buf(),
                position: contents
                    .find(line.as_str())
                    .map(|line_offset| TextPosition::of(contents, line_offset + index)),
                source: dotenv_error,
            },
            _ => EnvFileError::Load {
                path: path.to_path_buf(),
                source: dotenv_error,
            },
        })?;
        values.entry(name).or_insert(value);
    }
    Ok(EnvFile { values })
}

/// Conteúdo do `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub gsi_port: u16,
    pub gsi_token: String,
    /// Bibliotecas Steam fora dos caminhos padrão.
    pub extra_libraries: Vec<PathBuf>,
    /// Arquivo onde os eventos GSI do Dota são gravados. Só vem do ambiente,
    /// nunca é salvo no `config.toml`.
    #[serde(skip)]
    pub gsi_log: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gsi_port: DEFAULT_GSI_PORT,
            gsi_token: String::new(),
            extra_libraries: vec![PathBuf::from("/mnt/jogos/SteamLibrary")],
            gsi_log: None,
        }
    }
}

/// Caminho do `config.toml` no diretório de configuração do usuário (XDG).
pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "dota-companion")
        .map(|project_dirs| project_dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("dota-companion.toml"))
}

/// Carrega o arquivo, gera o token se faltar e aplica as variáveis obtidas
/// por `lookup` (ver `apply_environment`).
pub fn load_or_init(
    path: &Path,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<AppConfig, ConfigError> {
    let mut config = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|toml_error| ConfigError::Parse {
            path: path.to_path_buf(),
            position: toml_error
                .span()
                .map(|span| TextPosition::of(&text, span.start)),
            source: Box::new(toml_error),
        })?,
        Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => AppConfig::default(),
        Err(io_error) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source: io_error,
            });
        }
    };
    if config.gsi_token.is_empty() {
        config.gsi_token = generate_token()?;
        save(path, &config)?;
    }

    apply_environment(&mut config, lookup);
    Ok(config)
}

/// Sobrescreve a configuração com as variáveis de ambiente:
/// `DOTA_COMPANION_GSI_PORT`, `DOTA_COMPANION_EXTRA_LIBRARIES` (caminhos
/// separados por `:`) e `DOTA_COMPANION_GSI_LOG` (arquivo de log dos eventos).
pub fn apply_environment(config: &mut AppConfig, lookup: impl Fn(&str) -> Option<String>) {
    if let Some(port) =
        lookup("DOTA_COMPANION_GSI_PORT").and_then(|port_text| port_text.parse().ok())
    {
        config.gsi_port = port;
    }
    if let Some(extra_paths) = lookup("DOTA_COMPANION_EXTRA_LIBRARIES") {
        config.extra_libraries.extend(
            std::env::split_paths(&extra_paths).filter(|library| !library.as_os_str().is_empty()),
        );
    }
    if let Some(log_path) = lookup("DOTA_COMPANION_GSI_LOG").filter(|path| !path.is_empty()) {
        config.gsi_log = Some(PathBuf::from(log_path));
    }
}

/// Grava a configuração com permissão 0600.
pub fn save(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    let write_error = |source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(config_dir) = path.parent() {
        fs::create_dir_all(config_dir).map_err(write_error)?;
    }
    let text = toml::to_string_pretty(config).map_err(ConfigError::Serialize)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(write_error)?;
    file.write_all(text.as_bytes()).map_err(write_error)
}

fn generate_token() -> Result<String, ConfigError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(ConfigError::TokenGeneration)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/config_tests.rs"]
mod tests;
