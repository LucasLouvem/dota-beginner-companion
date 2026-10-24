//! Adaptador de arquivos para ler, gravar e fazer backup dos `.cfg` do GSI.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::ConfigFiles;

/// Implementação de `ConfigFiles` no sistema de arquivos local.
pub struct LocalConfigFiles;

impl ConfigFiles for LocalConfigFiles {
    fn read(&self, path: &Path) -> io::Result<Option<String>> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(io_error) => Err(io_error),
        }
    }

    fn ensure_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)
    }

    /// Escreve em arquivo temporário e renomeia, para nunca deixar um `.cfg`
    /// pela metade. Se algo falhar, o temporário é removido e o erro
    /// devolvido é sempre o da gravação ou da renomeação.
    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        let partial_path = path.with_extension("cfg.tmp");
        fs::write(&partial_path, contents)
            .and_then(|()| fs::rename(&partial_path, path))
            .inspect_err(|_| discard_partial(&partial_path))
    }

    fn backup(&self, path: &Path) -> io::Result<PathBuf> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut backup = path.with_file_name(format!("{file_name}.bak-{timestamp}"));
        let mut attempt = 1;
        while backup.exists() {
            backup = path.with_file_name(format!("{file_name}.bak-{timestamp}-{attempt}"));
            attempt += 1;
        }
        fs::copy(path, &backup)?;
        Ok(backup)
    }
}

/// Remove o temporário de uma gravação que falhou. Uma falha aqui é
/// secundária: vai para o log, sem substituir o erro principal. O log leva
/// só o nome do arquivo, porque o caminho completo inclui o nome do usuário.
fn discard_partial(partial_path: &Path) {
    match fs::remove_file(partial_path) {
        Ok(()) => {}
        // O temporário nem chegou a ser criado.
        Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => {}
        Err(cleanup_error) => tracing::warn!(
            file = %partial_path.file_name().unwrap_or_default().to_string_lossy(),
            error = %cleanup_error,
            "could not remove partial GSI config file"
        ),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/config_files_tests.rs"]
mod tests;
