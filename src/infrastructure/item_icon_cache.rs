//! Imagens dos itens guardadas na pasta de cache do usuário
//! (`~/.cache/dota-companion/item-icons/<item>.png`), para baixar cada uma
//! só uma vez.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::application::ports::{ItemIconError, ItemIconStore};

/// Pasta das imagens dentro do cache do aplicativo.
const ICONS_DIR: &str = "item-icons";

/// Imagens em arquivos PNG, uma por item.
pub struct ItemIconCache {
    dir: PathBuf,
}

impl ItemIconCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Pasta padrão, no cache do usuário; `None` quando o sistema não informa
    /// a pasta pessoal.
    pub fn default_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "dota-companion")
            .map(|project_dirs| project_dirs.cache_dir().join(ICONS_DIR))
    }

    /// Arquivo do item. Só nomes internos simples (`item_` seguido de letras
    /// minúsculas, números e `_`) viram caminho, para um nome inesperado
    /// nunca sair da pasta.
    fn file_of(&self, item: &str) -> Result<PathBuf, ItemIconError> {
        let valid = item.strip_prefix("item_").is_some_and(|name| {
            !name.is_empty()
                && name.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        });
        if !valid {
            return Err(ItemIconError::UnexpectedPath { path: item.into() });
        }
        Ok(self.dir.join(format!("{item}.png")))
    }
}

impl ItemIconStore for ItemIconCache {
    fn load(&self, item: &str) -> Result<Option<Vec<u8>>, ItemIconError> {
        let path = self.file_of(item)?;
        match fs::read(&path) {
            Ok(png) => Ok(Some(png)),
            Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(io_error) => Err(cache_error(path, io_error)),
        }
    }

    /// Grava num arquivo temporário e renomeia, para nunca deixar uma imagem
    /// pela metade.
    fn save(&self, item: &str, png: &[u8]) -> Result<(), ItemIconError> {
        let path = self.file_of(item)?;
        let partial = path.with_extension("png.tmp");
        fs::create_dir_all(&self.dir)
            .and_then(|()| fs::write(&partial, png))
            .and_then(|()| fs::rename(&partial, &path))
            .inspect_err(|_| discard_partial(&partial))
            .map_err(|io_error| cache_error(path, io_error))
    }
}

/// Remove o temporário de uma gravação que falhou. Se a remoção também
/// falhar, o erro que importa é o da gravação, já devolvido.
fn discard_partial(partial: &Path) {
    match fs::remove_file(partial) {
        Ok(()) => {}
        // O temporário nem chegou a ser criado.
        Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => {}
        Err(cleanup_error) => tracing::warn!(
            file = %partial.file_name().unwrap_or_default().to_string_lossy(),
            error = %cleanup_error,
            "could not remove partial item icon file"
        ),
    }
}

fn cache_error(path: PathBuf, io_error: io::Error) -> ItemIconError {
    ItemIconError::Cache {
        path,
        source: Arc::new(io_error),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/item_icon_cache_tests.rs"]
mod tests;
