//! Executa processos de teste com saída capturada e tempo limite.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Espera no máximo dez segundos; mata e recolhe o processo se ele travar.
/// Os processos usados aqui produzem apenas mensagens curtas, sem encher os pipes.
pub fn output_with_timeout(command: &mut Command) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("o subprocesso deve iniciar");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().expect("o subprocesso deve ser encerrado");
            let output = child.wait_with_output().unwrap();
            panic!("subprocesso excedeu o tempo limite: {:?}", output.status);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
