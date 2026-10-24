//! Servidor GSI real ligado ao núcleo: eventos aceitos, recusados, teste de
//! conexão e falha de bind chegam ao `AppSnapshot`.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};

use dota_companion::application::error::{DiagnosticCode, Redactor, ToDiagnostic};
use dota_companion::application::event_bus::UiCommand;
use dota_companion::application::state::{ConnectionTest, ServerState};
use dota_companion::domain::gsi_config::GsiEndpoint;
use dota_companion::gsi::server::serve;
use serde_json::{Value, json};

use super::harness::RunningCore;

const TOKEN: &str = "integration-token";

fn endpoint(port: u16) -> GsiEndpoint {
    GsiEndpoint {
        port,
        token: TOKEN.into(),
    }
}

/// Inicia o núcleo e o servidor GSI em porta livre e devolve o endereço ouvido.
async fn start_listening() -> (RunningCore, SocketAddr) {
    let mut core = RunningCore::start(endpoint(0), None);
    tokio::spawn(serve(0, TOKEN.into(), core.events.clone()));
    let snapshot = core
        .wait_for(|snapshot| matches!(snapshot.gsi.server, ServerState::Listening(_)))
        .await;
    let ServerState::Listening(address) = snapshot.gsi.server else {
        unreachable!("a condição acima garante Listening");
    };
    (core, address)
}

async fn post_json(address: SocketAddr, body: &Value) -> reqwest::StatusCode {
    reqwest::Client::new()
        .post(format!("http://{address}/"))
        .json(body)
        .send()
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn dota_event_with_valid_token_reaches_snapshot_without_auth_block() {
    let (mut core, address) = start_listening().await;

    let status = post_json(
        address,
        &json!({ "auth": { "token": TOKEN }, "map": { "game_state": "DOTA_GAMERULES_STATE_PRE_GAME" } }),
    )
    .await;
    assert_eq!(status, reqwest::StatusCode::OK);

    let snapshot = core
        .wait_for(|snapshot| snapshot.gsi.dota_events == 1)
        .await;
    let event = snapshot
        .gsi
        .last_dota_event
        .expect("o evento do Dota fica no estado");
    let payload: Value = serde_json::from_str(&event.raw).unwrap();
    assert_eq!(
        payload,
        json!({ "map": { "game_state": "DOTA_GAMERULES_STATE_PRE_GAME" } })
    );
    assert_eq!(snapshot.gsi.rejected, 0);
    assert!(snapshot.gsi.last_test_event.is_none());
}

#[tokio::test]
async fn event_with_wrong_token_is_counted_as_rejected_and_not_as_dota_event() {
    let (mut core, address) = start_listening().await;

    let status = post_json(address, &json!({ "auth": { "token": "wrong" } })).await;
    assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);

    let snapshot = core.wait_for(|snapshot| snapshot.gsi.rejected == 1).await;
    assert_eq!(snapshot.gsi.dota_events, 0);
    assert!(snapshot.gsi.last_dota_event.is_none());
}

/// Porta livre no momento da chamada, para o núcleo conhecer a porta antes
/// do servidor abrir.
fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn connection_test_passes_and_is_stored_apart_from_dota_events() {
    let port = free_port();
    let mut core = RunningCore::start(endpoint(port), None);
    tokio::spawn(serve(port, TOKEN.into(), core.events.clone()));
    core.wait_for(|snapshot| matches!(snapshot.gsi.server, ServerState::Listening(_)))
        .await;

    core.commands.send(UiCommand::TestConnection).unwrap();

    let snapshot = core
        .wait_for(|snapshot| {
            matches!(snapshot.connection_test, Some(ConnectionTest::Passed))
                && snapshot.gsi.last_test_event.is_some()
        })
        .await;
    assert_eq!(snapshot.gsi.dota_events, 0);
    assert!(snapshot.gsi.last_dota_event.is_none());
    let test_event = snapshot.gsi.last_test_event.unwrap();
    let payload: Value = serde_json::from_str(&test_event.raw).unwrap();
    assert_eq!(
        payload,
        json!({
            "dota_companion_test": true,
            "provider": { "name": "Dota Beginner Companion (teste)" },
        })
    );
}

#[tokio::test]
async fn occupied_port_is_shown_as_gsi_bind_failed() {
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = occupied.local_addr().unwrap().port();
    let mut core = RunningCore::start(endpoint(port), None);

    tokio::spawn(serve(port, TOKEN.into(), core.events.clone()));

    let snapshot = core
        .wait_for(|snapshot| matches!(snapshot.gsi.server, ServerState::Failed(_)))
        .await;
    let ServerState::Failed(server_error) = &snapshot.gsi.server else {
        unreachable!("a condição acima garante Failed");
    };
    let diagnostic = server_error.to_diagnostic(&Redactor::new([TOKEN]));
    assert_eq!(diagnostic.code, DiagnosticCode::GsiBindFailed);
    assert_eq!(diagnostic.cause.args, vec![port.to_string()]);
}
