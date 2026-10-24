//! Exercita o executável de envio sintético com arquivos isolados e servidor real.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

use dota_companion::application::event_bus::{CoreEvent, RejectReason};
use dota_companion::gsi::server::{TEST_MARKER, router};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use super::process::output_with_timeout;

const TOKEN: &str = "synthetic-cli-secret-2917";

fn example_binary() -> PathBuf {
    // O Cargo compila os exemplos junto com `cargo test`.
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/send_test_event")
}

fn send_event(
    config_port: u16,
    token: &str,
    env_port: Option<u16>,
    process_port: Option<u16>,
) -> Output {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_dir = temp_dir.path().join("config/dota-companion");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("gsi_port = {config_port}\ngsi_token = {token:?}\n"),
    )
    .unwrap();
    if let Some(env_port) = env_port {
        fs::write(
            temp_dir.path().join(".env"),
            format!("DOTA_COMPANION_GSI_PORT={env_port}\n"),
        )
        .unwrap();
    }
    let mut command = Command::new(example_binary());
    command
        .current_dir(temp_dir.path())
        .env_clear()
        .env("HOME", temp_dir.path())
        .env("XDG_CONFIG_HOME", temp_dir.path().join("config"));
    if let Some(process_port) = process_port {
        command.env("DOTA_COMPANION_GSI_PORT", process_port.to_string());
    }
    let output = output_with_timeout(&mut command);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains(token),
        "o terminal não pode expor o token"
    );
    output
}

async fn start_server() -> (u16, mpsc::Receiver<CoreEvent>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        axum::serve(listener, router(TOKEN.into(), sender))
            .await
            .unwrap();
    });
    (port, receiver, task)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_sends_one_synthetic_event_with_each_configuration_priority() {
    let (port, mut events, task) = start_server().await;
    for (config_port, env_port, process_port) in [
        (1, Some(2), Some(port)),
        (1, Some(port), None),
        (port, None, None),
    ] {
        let output = send_event(config_port, TOKEN, env_port, process_port);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "Evento de teste enviado com sucesso.\n"
        );
        assert!(output.stderr.is_empty());
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        let CoreEvent::GsiAccepted { raw, is_test, .. } = event else {
            panic!("esperava um evento aceito");
        };
        assert!(is_test);
        assert_eq!(
            serde_json::from_str::<Value>(&raw).unwrap(),
            json!({ TEST_MARKER: true, "provider": { "name": "Dota Beginner Companion (teste)" } })
        );
        assert!(events.try_recv().is_err());
    }
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_wrong_token_exits_with_failure_and_exact_diagnostic_code() {
    let (port, mut events, task) = start_server().await;
    let output = send_event(port, "wrong-cli-secret-5812", None, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Código: CONNECTION_TEST_TOKEN_REJECTED"));
    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        CoreEvent::GsiRejected {
            reason: RejectReason::InvalidToken,
            ..
        }
    ));
    assert!(events.try_recv().is_err());
    task.abort();
}

#[test]
fn cli_unavailable_server_exits_with_failure_and_connection_diagnostic() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let output = send_event(port, TOKEN, None, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Código: CONNECTION_TEST_UNREACHABLE")
    );
}
