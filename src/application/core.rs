//! Laço principal: recebe eventos dos adaptadores e comandos da interface,
//! executa os casos de uso e publica o novo `AppSnapshot`.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{mpsc, watch};

use crate::domain::gsi_config::GsiEndpoint;

use super::event_bus::{CoreEvent, UiCommand};
use super::ports::{ConfigFiles, ConnectionTester, DotaLocator, GsiRecorder, ProcessProbe};
use super::state::{AppSnapshot, ConnectionTest};
use super::use_cases::install_gsi_config::install_gsi_config;
use super::use_cases::record_gsi_event::record_gsi_event;
use super::use_cases::record_gsi_payload::{apply_record_result, payload_to_record};
use super::use_cases::run_diagnostics::run_diagnostics;

const DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(10);

/// Adaptadores e configuração usados pelos casos de uso do núcleo.
#[derive(Clone)]
pub struct CoreDependencies {
    pub locator: Arc<dyn DotaLocator>,
    pub probe: Arc<dyn ProcessProbe>,
    pub files: Arc<dyn ConfigFiles>,
    pub tester: Arc<dyn ConnectionTester>,
    /// Presente quando a gravação de eventos em arquivo está ativa.
    pub recorder: Option<Arc<dyn GsiRecorder>>,
    pub endpoint: GsiEndpoint,
}

/// Processa eventos do coletor GSI e comandos da interface até os canais fecharem,
/// rodando o diagnóstico a cada `DIAGNOSTICS_INTERVAL`.
pub async fn run(
    dependencies: CoreDependencies,
    mut commands: mpsc::UnboundedReceiver<UiCommand>,
    mut events: mpsc::Receiver<CoreEvent>,
    state: Arc<watch::Sender<AppSnapshot>>,
) {
    let mut ticker = tokio::time::interval(DIAGNOSTICS_INTERVAL);
    let payload_sender = dependencies
        .recorder
        .clone()
        .map(|recorder| spawn_recorder(recorder, Arc::clone(&state)));
    loop {
        tokio::select! {
            Some(event) = events.recv() => {
                if let (Some(sender), Some(payload)) = (&payload_sender, payload_to_record(&event)) {
                    let _ = sender.send(payload);
                }
                state.send_modify(|snapshot| record_gsi_event(&mut snapshot.gsi, event));
            }
            Some(command) = commands.recv() => handle_command(&dependencies, &state, command),
            _ = ticker.tick() => spawn_diagnostics(&dependencies, &state),
            else => break,
        }
    }
}

fn handle_command(
    dependencies: &CoreDependencies,
    state: &Arc<watch::Sender<AppSnapshot>>,
    command: UiCommand,
) {
    match command {
        UiCommand::RunDiagnostics => spawn_diagnostics(dependencies, state),
        UiCommand::TestConnection => {
            state.send_modify(|snapshot| snapshot.connection_test = Some(ConnectionTest::Running));
            let (dependencies, state) = (dependencies.clone(), Arc::clone(state));
            tokio::spawn(async move {
                let result = match dependencies
                    .tester
                    .send_test_event(&dependencies.endpoint)
                    .await
                {
                    Ok(()) => ConnectionTest::Passed,
                    Err(reason) => ConnectionTest::Failed(reason),
                };
                state.send_modify(|snapshot| snapshot.connection_test = Some(result));
            });
        }
        UiCommand::InstallGsiConfig {
            install_root,
            overwrite,
        } => {
            let (dependencies, state) = (dependencies.clone(), Arc::clone(state));
            tokio::task::spawn_blocking(move || {
                let outcome = install_gsi_config(
                    dependencies.files.as_ref(),
                    &install_root,
                    &dependencies.endpoint,
                    overwrite,
                );
                let report = build_diagnostics(&dependencies);
                state.send_modify(|snapshot| {
                    snapshot.last_install = Some(outcome);
                    snapshot.install_attempt += 1;
                    snapshot.diagnostics = Some(report);
                });
            });
        }
    }
}

fn build_diagnostics(
    dependencies: &CoreDependencies,
) -> crate::domain::diagnostics::DiagnosticReport {
    run_diagnostics(
        dependencies.locator.as_ref(),
        dependencies.probe.as_ref(),
        dependencies.files.as_ref(),
        &dependencies.endpoint,
    )
}

fn spawn_diagnostics(dependencies: &CoreDependencies, state: &Arc<watch::Sender<AppSnapshot>>) {
    let (dependencies, state) = (dependencies.clone(), Arc::clone(state));
    tokio::task::spawn_blocking(move || {
        let report = build_diagnostics(&dependencies);
        state.send_modify(|snapshot| snapshot.diagnostics = Some(report));
    });
}

/// Grava os eventos em uma única tarefa, um de cada vez, para manter no
/// arquivo a mesma ordem de chegada.
fn spawn_recorder(
    recorder: Arc<dyn GsiRecorder>,
    state: Arc<watch::Sender<AppSnapshot>>,
) -> mpsc::UnboundedSender<(SystemTime, Arc<str>)> {
    let (payload_sender, mut payload_receiver) =
        mpsc::unbounded_channel::<(SystemTime, Arc<str>)>();
    tokio::spawn(async move {
        while let Some((received_at, raw)) = payload_receiver.recv().await {
            let recorder = Arc::clone(&recorder);
            let Ok(result) =
                tokio::task::spawn_blocking(move || recorder.record(received_at, &raw)).await
            else {
                break;
            };
            state.send_modify(|snapshot| {
                if let Some(log_status) = &mut snapshot.gsi_log {
                    apply_record_result(log_status, result);
                }
            });
        }
    });
    payload_sender
}
