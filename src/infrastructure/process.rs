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
                .all(|b| b.is_ascii_digit());
            is_pid
                && fs::read_to_string(entry.path().join("comm"))
                    .is_ok_and(|comm| DOTA_PROCESS_NAMES.contains(&comm.trim()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_proc(processes: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (pid, comm) in processes {
            let dir = tmp.path().join(pid);
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
        }
        tmp
    }

    #[test]
    fn detects_dota_by_process_name() {
        let tmp = fake_proc(&[("1", "systemd"), ("4242", "dota2")]);
        assert!(ProcProbe::new(tmp.path().into()).dota_running());
    }

    #[test]
    fn ignores_other_processes_and_non_pid_entries() {
        let tmp = fake_proc(&[("1", "systemd"), ("self", "dota2")]);
        assert!(!ProcProbe::new(tmp.path().into()).dota_running());
    }
}
