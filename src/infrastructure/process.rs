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
mod tests {
    use super::*;

    /// Cria um `/proc` falso com uma pasta por entrada e o arquivo `comm`.
    fn fake_proc(entries: &[(&str, &str)]) -> tempfile::TempDir {
        let proc_root = tempfile::tempdir().unwrap();
        for (entry_name, process_name) in entries {
            let entry_dir = proc_root.path().join(entry_name);
            fs::create_dir(&entry_dir).unwrap();
            fs::write(entry_dir.join("comm"), format!("{process_name}\n")).unwrap();
        }
        proc_root
    }

    #[test]
    fn reports_running_when_a_process_is_named_dota2() {
        let proc_root = fake_proc(&[("1", "systemd"), ("4242", "dota2")]);
        assert!(ProcProbe::new(proc_root.path().into()).dota_running());
    }

    #[test]
    fn reports_not_running_when_no_process_is_named_dota2() {
        let proc_root = fake_proc(&[("1", "systemd"), ("2", "steam")]);
        assert!(!ProcProbe::new(proc_root.path().into()).dota_running());
    }

    #[test]
    fn ignores_entries_that_are_not_process_ids() {
        let proc_root = fake_proc(&[("self", "dota2")]);
        assert!(!ProcProbe::new(proc_root.path().into()).dota_running());
    }
}
