//! Confere a saída real de pânico sem alterar o hook do processo dos testes.

use std::process::Command;

use dota_companion::infrastructure::logging::install_panic_hook;

use super::process::output_with_timeout;

const SECRET: &str = "simulated-panic-secret-9381";
const CHILD_FLAG: &str = "DOTA_COMPANION_PANIC_PROBE";

#[test]
fn panic_child_probe() {
    if std::env::var_os(CHILD_FLAG).is_none() {
        return;
    }
    install_panic_hook();
    std::thread::Builder::new()
        .name(SECRET.into())
        .spawn(|| panic!("payload {SECRET}"))
        .unwrap()
        .join()
        .expect("o pânico da thread deve continuar sendo propagado");
}

#[test]
fn panic_output_keeps_location_without_payload_thread_name_or_backtrace() {
    let output = output_with_timeout(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "integration::panic_output::panic_child_probe",
                "--nocapture",
            ])
            .env_clear()
            .env(CHILD_FLAG, "1")
            .env("RUST_BACKTRACE", "full"),
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{stderr}");
    for line in lines {
        let location = line
            .strip_prefix("Falha interna (pânico) em panic_output.rs:")
            .and_then(|location| location.strip_suffix('.'))
            .expect("cada pânico deve mostrar só a mensagem fixa e a localização");
        let (line, column) = location.split_once(':').unwrap();
        assert!(line.parse::<u32>().unwrap() > 0);
        assert!(column.parse::<u32>().unwrap() > 0);
    }
    for text in [&stdout, &stderr] {
        assert!(!text.contains(SECRET));
        assert!(!text.contains("payload"));
        assert!(!text.contains("stack backtrace"));
        assert!(!text.contains("thread '"));
    }
}
