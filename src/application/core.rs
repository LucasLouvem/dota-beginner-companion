//! Laço principal: recebe eventos dos adaptadores e comandos da interface,
//! executa os casos de uso e publica o novo `AppSnapshot`.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};

use crate::domain::gsi_config::GsiEndpoint;

use super::event_bus::{CoreEvent, UiCommand};
use super::ports::{ConfigFiles, ConnectionTester, DotaLocator, ProcessProbe};
use super::state::{AppSnapshot, ConnectionTest};
use super::use_cases::install_gsi_config::install_gsi_config;
use super::use_cases::record_gsi_event::record_gsi_event;
use super::use_cases::run_diagnostics::run_diagnostics;

const DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(10);

/// Adaptadores e configuração usados pelos casos de uso do núcleo.
#[derive(Clone)]
pub struct CoreDeps {
    pub locator: Arc<dyn DotaLocator>,
    pub probe: Arc<dyn ProcessProbe>,
    pub files: Arc<dyn ConfigFiles>,
    pub tester: Arc<dyn ConnectionTester>,
    pub endpoint: GsiEndpoint,
}

pub async fn run(
    deps: CoreDeps,
    mut commands: mpsc::UnboundedReceiver<UiCommand>,
    mut events: mpsc::Receiver<CoreEvent>,
    state: Arc<watch::Sender<AppSnapshot>>,
) {
    let mut ticker = tokio::time::interval(DIAGNOSTICS_INTERVAL);
    loop {
        tokio::select! {
            Some(event) = events.recv() => {
                state.send_modify(|s| record_gsi_event(&mut s.gsi, event));
            }
            Some(command) = commands.recv() => handle_command(&deps, &state, command),
            _ = ticker.tick() => spawn_diagnostics(&deps, &state),
            else => break,
        }
    }
}

fn handle_command(deps: &CoreDeps, state: &Arc<watch::Sender<AppSnapshot>>, command: UiCommand) {
    match command {
        UiCommand::RunDiagnostics => spawn_diagnostics(deps, state),
        UiCommand::TestConnection => {
            state.send_modify(|s| s.connection_test = Some(ConnectionTest::Running));
            let (deps, state) = (deps.clone(), Arc::clone(state));
            tokio::spawn(async move {
                let result = match deps.tester.send_test_event(&deps.endpoint).await {
                    Ok(()) => ConnectionTest::Passed,
                    Err(e) => ConnectionTest::Failed(e),
                };
                state.send_modify(|s| s.connection_test = Some(result));
            });
        }
        UiCommand::InstallGsiConfig {
            install_root,
            overwrite,
        } => {
            let (deps, state) = (deps.clone(), Arc::clone(state));
            tokio::task::spawn_blocking(move || {
                let outcome = install_gsi_config(
                    deps.files.as_ref(),
                    &install_root,
                    &deps.endpoint,
                    overwrite,
                );
                let report = diagnostics(&deps);
                state.send_modify(|s| {
                    s.last_install = Some(outcome);
                    s.install_attempt += 1;
                    s.diagnostics = Some(report);
                });
            });
        }
    }
}

fn diagnostics(deps: &CoreDeps) -> crate::domain::diagnostics::DiagnosticReport {
    run_diagnostics(
        deps.locator.as_ref(),
        deps.probe.as_ref(),
        deps.files.as_ref(),
        &deps.endpoint,
    )
}

fn spawn_diagnostics(deps: &CoreDeps, state: &Arc<watch::Sender<AppSnapshot>>) {
    let (deps, state) = (deps.clone(), Arc::clone(state));
    tokio::task::spawn_blocking(move || {
        let report = diagnostics(&deps);
        state.send_modify(|s| s.diagnostics = Some(report));
    });
}
