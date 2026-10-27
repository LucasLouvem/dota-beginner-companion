//! Verifica se o Dota e a Steam estão abertos pelo nome dos processos em `/proc/*/comm`.
//! Não lê memória nem se conecta ao processo.

use std::fs;
use std::path::PathBuf;

use crate::application::ports::ProcessProbe;

const DOTA_PROCESS_NAMES: &[&str] = &["dota2", "dota2.exe"];
// Só o cliente: `steamwebhelper` e `steam-runtime-l` podem sobreviver a ele.
const STEAM_PROCESS_NAMES: &[&str] = &["steam"];

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

impl ProcProbe {
    /// Diz se algum processo tem um dos `names` em `comm`.
    fn any_process_named(&self, names: &[&str]) -> bool {
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
                    .is_ok_and(|comm| names.contains(&comm.trim()))
        })
    }
}

impl ProcessProbe for ProcProbe {
    fn dota_running(&self) -> bool {
        self.any_process_named(DOTA_PROCESS_NAMES)
    }

    fn steam_running(&self) -> bool {
        self.any_process_named(STEAM_PROCESS_NAMES)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/process_tests.rs"]
mod tests;
