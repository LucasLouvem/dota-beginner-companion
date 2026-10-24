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
    /// pela metade.
    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        let partial_path = path.with_extension("cfg.tmp");
        fs::write(&partial_path, contents)?;
        fs::rename(&partial_path, path).inspect_err(|_| {
            let _ = fs::remove_file(&partial_path);
        })
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

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/config_files_tests.rs"]
mod tests;
