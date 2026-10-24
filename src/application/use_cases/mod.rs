//! Casos de uso da aplicação, um por arquivo.

pub mod install_gsi_config;
pub mod record_gsi_event;
pub mod run_diagnostics;

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashMap;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use crate::application::ports::ConfigFiles;

    /// Sistema de arquivos em memória para testar a política dos casos de uso.
    #[derive(Default)]
    pub struct MemoryFiles {
        files: Mutex<HashMap<PathBuf, String>>,
    }

    impl MemoryFiles {
        pub fn put(&self, path: &Path, contents: &str) {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), contents.to_owned());
        }

        pub fn get(&self, path: &Path) -> Option<String> {
            self.files.lock().unwrap().get(path).cloned()
        }
    }

    impl ConfigFiles for MemoryFiles {
        fn read(&self, path: &Path) -> io::Result<Option<String>> {
            Ok(self.get(path))
        }

        fn ensure_dir(&self, _dir: &Path) -> io::Result<()> {
            Ok(())
        }

        fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
            self.put(path, contents);
            Ok(())
        }

        fn backup(&self, path: &Path) -> io::Result<PathBuf> {
            let backup = path.with_extension("cfg.bak");
            let contents = self.get(path).ok_or(io::ErrorKind::NotFound)?;
            self.put(&backup, &contents);
            Ok(backup)
        }
    }
}
