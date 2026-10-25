//! Exercita o executável de reenvio do log GSI com arquivos isolados e servidor real.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use dota_companion::application::event_bus::CoreEvent;
use dota_companion::domain::game::{GamePhase, HeroPick};
use dota_companion::gsi::server::router;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use super::process::output_with_timeout;

const TOKEN: &str = "synthetic-replay-secret-6301";
/// Valor anonimizado do nome do jogador nas fixtures.
const FIXTURE_PLAYER_NAME: &str = "Jogador Anonimo";

fn example_binary() -> PathBuf {
    // O Cargo compila os exemplos junto com `cargo test`.
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/replay_gsi_log")
}

fn zeus_fixture() -> String {
    fs::read_to_string(format!(
        "{}/tests/fixtures/gsi/zeus_bot_match.jsonl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Roda o reenvio num diretório temporário com `config.toml` e log próprios.
fn replay(port: u16, log_contents: Option<&str>) -> Output {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_dir = temp_dir.path().join("config/dota-companion");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("gsi_port = {port}\ngsi_token = {TOKEN:?}\n"),
    )
    .unwrap();
    let log_path = temp_dir.path().join("replay.jsonl");
    if let Some(contents) = log_contents {
        fs::write(&log_path, contents).unwrap();
    }
    let output = output_with_timeout(
        Command::new(example_binary())
            .arg(Path::new("replay.jsonl"))
            .args(["--interval-ms", "0"])
            .current_dir(temp_dir.path())
            .env_clear()
            .env("HOME", temp_dir.path())
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config")),
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains(TOKEN),
        "o terminal não pode expor o token"
    );
    assert!(
        !combined.contains(FIXTURE_PLAYER_NAME),
        "o terminal não pode expor dados do payload"
    );
    output
}

async fn start_server() -> (u16, mpsc::Receiver<CoreEvent>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = mpsc::channel(32);
    let task = tokio::spawn(async move {
        axum::serve(listener, router(TOKEN.into(), sender))
            .await
            .unwrap();
    });
    (port, receiver, task)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replays_every_logged_event_as_dota_event_in_order() {
    let (port, mut events, task) = start_server().await;
    let log = format!("{}linha que não é JSON\n", zeus_fixture());

    let output = replay(port, Some(&log));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eventos reenviados — HTTP 200: 9; linhas ignoradas: 1.\n"
    );
    let mut games = Vec::new();
    for _ in 0..9 {
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        let CoreEvent::GsiAccepted { is_test, game, .. } = event else {
            panic!("esperava um evento aceito: {event:?}");
        };
        assert!(!is_test);
        games.push(*game.expect("eventos do Dota são normalizados"));
    }
    assert_eq!(games[0].phase, None);
    assert_eq!(games[1].phase, Some(GamePhase::HeroSelection));
    assert_eq!(
        games[2].hero,
        HeroPick::Picked {
            name: "npc_dota_hero_zuus".into(),
            id: Some(22),
        }
    );
    assert_eq!(games[8].phase, Some(GamePhase::InProgress));
    assert!(events.try_recv().is_err());
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_log_file_exits_with_failure_and_diagnostic() {
    let (port, mut events, task) = start_server().await;

    let output = replay(port, None);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("Reenvio do log GSI falhou\n"),
        "{stderr}"
    );
    assert!(stderr.contains("Código: UNCLASSIFIED_ERROR"), "{stderr}");
    assert!(events.try_recv().is_err());
    task.abort();
}
