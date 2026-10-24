//! Testes unitários de `src/infrastructure/process.rs`.

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
