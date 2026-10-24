//! Conteúdo do arquivo de configuração do Game State Integration (GSI).
//!
//! O Dota lê todos os arquivos `gamestate_integration_*.cfg` da pasta
//! `game/dota/cfg/gamestate_integration/` e envia POSTs JSON para a `uri` de
//! cada um.

use super::keyvalues::{self, KeyValue, KeyValuesError};

/// Nome do arquivo `.cfg` criado por este aplicativo.
pub const CONFIG_FILE_NAME: &str = "gamestate_integration_dota_companion.cfg";
const SECTION_NAME: &str = "Dota Beginner Companion";

/// Blocos de dados pedidos ao Dota. `draft` e `wearables` ficam de fora:
/// não são necessários e o draft pode trazer escolhas de outros jogadores.
pub const REQUESTED_DATA: &[&str] = &["provider", "map", "player", "hero", "abilities", "items"];

/// Porta e token que o Dota usa para enviar eventos ao aplicativo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsiEndpoint {
    pub port: u16,
    pub token: String,
}

impl GsiEndpoint {
    /// Sempre `127.0.0.1`: o servidor nunca escuta em outras interfaces.
    pub fn uri(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
}

/// Gera o conteúdo do arquivo `.cfg` para o endereço e token informados.
pub fn render(endpoint: &GsiEndpoint) -> String {
    let mut data_lines = String::new();
    for block in REQUESTED_DATA {
        data_lines.push_str(&format!("        \"{block}\" \"1\"\n"));
    }
    format!(
        "// Gerado pelo Dota Beginner Companion.\n\
         // Envia o estado do jogo somente para este computador (127.0.0.1).\n\
         \"{SECTION_NAME}\"\n\
         {{\n\
         \x20   \"uri\" \"{uri}\"\n\
         \x20   \"timeout\" \"5.0\"\n\
         \x20   \"buffer\" \"0.1\"\n\
         \x20   \"throttle\" \"0.5\"\n\
         \x20   \"heartbeat\" \"30.0\"\n\
         \x20   \"data\"\n\
         \x20   {{\n\
         {data_lines}\
         \x20   }}\n\
         \x20   \"auth\"\n\
         \x20   {{\n\
         \x20       \"token\" \"{token}\"\n\
         \x20   }}\n\
         }}\n",
        uri = endpoint.uri(),
        token = endpoint.token,
    )
}

/// Resultado da comparação de um `.cfg` existente com o esperado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigMatch {
    /// Mesmo endereço e token esperados pelo aplicativo.
    Matches,
    UriMismatch {
        found: Option<String>,
    },
    TokenMismatch,
    Unparsable(GsiConfigParseError),
}

/// Motivo de um `.cfg` do GSI não poder ser interpretado.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GsiConfigParseError {
    #[error("o conteúdo não é KeyValues válido")]
    KeyValues(#[source] KeyValuesError),
    #[error("nenhuma seção encontrada")]
    MissingSection,
}

/// Compara um arquivo existente com o que o aplicativo espera.
pub fn inspect(contents: &str, expected: &GsiEndpoint) -> ConfigMatch {
    let root = match keyvalues::parse(contents) {
        Ok(root) => root,
        Err(parse_error) => {
            return ConfigMatch::Unparsable(GsiConfigParseError::KeyValues(parse_error));
        }
    };
    let Some((_, section)) = root
        .entries()
        .iter()
        .find(|(_, value)| matches!(value, KeyValue::Section(_)))
    else {
        return ConfigMatch::Unparsable(GsiConfigParseError::MissingSection);
    };

    let uri = section.get("uri").and_then(KeyValue::as_text);
    if uri.map(|found_uri| found_uri.trim_end_matches('/'))
        != Some(expected.uri().trim_end_matches('/'))
    {
        return ConfigMatch::UriMismatch {
            found: uri.map(str::to_owned),
        };
    }
    let token = section
        .get("auth")
        .and_then(|auth| auth.get("token"))
        .and_then(KeyValue::as_text);
    if token != Some(expected.token.as_str()) {
        return ConfigMatch::TokenMismatch;
    }
    ConfigMatch::Matches
}

#[cfg(test)]
#[path = "../../tests/unit/domain/gsi_config_tests.rs"]
mod tests;
