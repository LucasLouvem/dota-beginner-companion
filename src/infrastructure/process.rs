//! Verifica se o Dota está aberto pelo nome dos processos em `/proc/*/comm`.
//! Não lê memória nem se conecta ao processo.

use std::fs;
use std::path::PathBuf;

use crate::application::ports::ProcessProbe;

const DOTA_PROCESS_NAMES: &[&str] = &["dota2", "dota2.exe"];

/// Sonda de processos baseada em `/proc`.
pub struct ProcProbe {
    proc_root: PathBuf,
}

impl ProcProbe {
    /// Usa `proc_root` no lugar de `/proc`.
    pub fn new(proc_root: PathBuf) -> Self {
        Self { proc_root }
    }
}

impl Default for ProcProbe {
    fn default() -> Self {
        Self::new(PathBuf::from("/proc"))
    }
}

impl ProcessProbe for ProcProbe {
    fn dota_running(&self) -> bool {
        let Ok(entries) = fs::read_dir(&self.proc_root) else {
            return false;
        };
        entries.flatten().any(|entry| {
            let is_pid = entry
                .file_name()
                .to_string_lossy()
                .bytes()
                .all(|byte| byte.is_ascii_digit());
            is_pid
                && fs::read_to_string(entry.path().join("comm"))
                    .is_ok_and(|comm| DOTA_PROCESS_NAMES.contains(&comm.trim()))
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/process_tests.rs"]
mod tests;
