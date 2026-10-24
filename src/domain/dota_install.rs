//! Instalação do Dota 2 em uma biblioteca Steam e seus caminhos.

use std::path::{Path, PathBuf};

use super::gsi_config::CONFIG_FILE_NAME;

/// ID do Dota 2 na Steam.
pub const DOTA_APP_ID: u32 = 570;
/// Nome da pasta do Dota 2 em `steamapps/common`.
pub const DOTA_DIR_NAME: &str = "dota 2 beta";

/// Uma instalação do Dota 2 encontrada em uma biblioteca Steam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DotaInstall {
    /// Raiz da biblioteca Steam (a pasta que contém `steamapps/`).
    pub library: PathBuf,
    /// `<biblioteca>/steamapps/common/dota 2 beta`.
    pub game_root: PathBuf,
    /// Existe o binário nativo Linux (`game/bin/linuxsteamrt64`).
    pub native_binary: bool,
    /// Existe prefixo Proton (`steamapps/compatdata/570`), sinal de que o jogo
    /// já foi executado pelo Proton em algum momento.
    pub proton_prefix: bool,
}

impl DotaInstall {
    /// Pasta do Dota dentro da biblioteca Steam informada.
    pub fn game_root_for(library: &Path) -> PathBuf {
        library.join("steamapps").join("common").join(DOTA_DIR_NAME)
    }

    /// Pasta onde o Dota lê os arquivos de configuração do GSI.
    pub fn gsi_dir(&self) -> PathBuf {
        self.game_root
            .join("game")
            .join("dota")
            .join("cfg")
            .join("gamestate_integration")
    }

    /// Caminho do arquivo `.cfg` deste aplicativo.
    pub fn gsi_config_path(&self) -> PathBuf {
        self.gsi_dir().join(CONFIG_FILE_NAME)
    }
}
