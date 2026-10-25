//! Laço principal: recebe eventos dos adaptadores e comandos da interface,
//! executa os casos de uso e publica o novo `AppSnapshot`.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{mpsc, watch};
use tokio::task::{JoinError, JoinHandle};

use crate::domain::gsi_config::GsiEndpoint;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_events::MatchTracker;

use super::diagnostics::DiagnosticReport;
use super::error::{ToDiagnostic, log_error};
use super::event_bus::{CoreEvent, UiCommand};
use super::ports::{ConfigFiles, ConnectionTester, DotaLocator, GsiRecorder, ProcessProbe};
use super::state::{
    AppSnapshot, ConnectionTest, CoreOperation, InstallOutcome, InterruptionCause, TaskInterrupted,
};
use super::use_cases::install_gsi_config::install_gsi_config;
use super::use_cases::record_gsi_event::record_gsi_event;
use super::use_cases::record_gsi_payload::{apply_record_result, payload_to_record};
use super::use_cases::run_diagnostics::run_diagnostics;
use super::use_cases::track_match::{record_manual_match_found, record_match_update};

const DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(10);

/// Evento do Dota a gravar no log: horário de chegada e JSON sem `auth`.
type RecordedPayload = (SystemTime, Arc<str>);

/// Adaptadores e configuração usados pelos casos de uso do núcleo.
#[derive(Clone)]
pub struct CoreDependencies {
    pub locator: Arc<dyn DotaLocator>,
    pub probe: Arc<dyn ProcessProbe>,
    pub files: Arc<dyn ConfigFiles>,
    pub tester: Arc<dyn ConnectionTester>,
    /// Presente quando a gravação de eventos em arquivo está ativa.
    pub recorder: Option<Arc<dyn GsiRecorder>>,
    /// Receitas usadas para reconhecer itens montados na linha do tempo.
    pub items: Arc<ItemCatalog>,
    pub endpoint: GsiEndpoint,
}

/// Processa eventos do coletor GSI e comandos da interface, rodando o
/// diagnóstico a cada `DIAGNOSTICS_INTERVAL`. O `ticker` mantém o laço ativo
/// mesmo com os canais fechados; ele só termina quando o runtime é encerrado.
pub async fn run(
    dependencies: CoreDependencies,
    mut commands: mpsc::UnboundedReceiver<UiCommand>,
    mut events: mpsc::Receiver<CoreEvent>,
    state: Arc<watch::Sender<AppSnapshot>>,
) {
    let mut ticker = tokio::time::interval(DIAGNOSTICS_INTERVAL);
    let mut payload_sender = dependencies
        .recorder
        .clone()
        .map(|recorder| spawn_recorder(recorder, Arc::clone(&state)));
    let mut match_tracker = MatchTracker::new(Arc::clone(&dependencies.items));
    loop {
        tokio::select! {
            Some(event) = events.recv() => {
                if let Some(payload) = payload_to_record(&event) {
                    send_to_recorder(&mut payload_sender, payload, &state);
                }
                let match_update = match &event {
                    CoreEvent::GsiAccepted { game: Some(game), at, .. } => {
                        let game = (**game).clone();
                        Some((game.clone(), match_tracker.observe(game), *at))
                    }
                    _ => None,
                };
                state.send_modify(|snapshot| {
                    record_gsi_event(&mut snapshot.gsi, event);
                    if let Some((game, match_events, at)) = match_update {
                        record_match_update(&mut snapshot.match_status, game, match_events, at);
                    }
                });
            }
            Some(command) = commands.recv() => handle_command(&dependencies, &state, command),
            _ = ticker.tick() => spawn_diagnostics(&dependencies, &state),
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
        UiCommand::MarkMatchFound => state.send_modify(|snapshot| {
            record_manual_match_found(&mut snapshot.match_status, SystemTime::now());
        }),
        UiCommand::TestConnection => {
            state.send_modify(|snapshot| snapshot.connection_test = Some(ConnectionTest::Running));
            let dependencies = dependencies.clone();
            let task = tokio::spawn(async move {
                dependencies
                    .tester
                    .send_test_event(&dependencies.endpoint)
                    .await
            });
            publish_when_done(
                task,
                CoreOperation::ConnectionTest,
                Arc::clone(state),
                |snapshot, result| {
                    snapshot.connection_test = Some(match result {
                        Ok(Ok(())) => ConnectionTest::Passed,
                        Ok(Err(test_error)) => ConnectionTest::Failed(test_error),
                        Err(interrupted) => ConnectionTest::Interrupted(interrupted),
                    });
                },
            );
        }
        UiCommand::InstallGsiConfig {
            install_root,
            overwrite,
        } => {
            let dependencies = dependencies.clone();
            let task = tokio::task::spawn_blocking(move || {
                let outcome = install_gsi_config(
                    dependencies.files.as_ref(),
                    &install_root,
                    &dependencies.endpoint,
                    overwrite,
                );
                (outcome, build_diagnostics(&dependencies))
            });
            publish_when_done(
                task,
                CoreOperation::InstallGsiConfig,
                Arc::clone(state),
                |snapshot, result| {
                    match result {
                        Ok((outcome, report)) => {
                            snapshot.last_install = Some(outcome);
                            snapshot.diagnostics = Some(report);
                            snapshot.diagnostics_interrupted = None;
                        }
                        Err(interrupted) => {
                            snapshot.last_install = Some(InstallOutcome::Interrupted(interrupted));
                        }
                    }
                    snapshot.install_attempt += 1;
                },
            );
        }
    }
}

fn build_diagnostics(dependencies: &CoreDependencies) -> DiagnosticReport {
    run_diagnostics(
        dependencies.locator.as_ref(),
        dependencies.probe.as_ref(),
        dependencies.files.as_ref(),
        &dependencies.endpoint,
    )
}

fn spawn_diagnostics(dependencies: &CoreDependencies, state: &Arc<watch::Sender<AppSnapshot>>) {
    let dependencies = dependencies.clone();
    let task = tokio::task::spawn_blocking(move || build_diagnostics(&dependencies));
    publish_when_done(
        task,
        CoreOperation::Diagnostics,
        Arc::clone(state),
        |snapshot, result| match result {
            Ok(report) => {
                snapshot.diagnostics = Some(report);
                snapshot.diagnostics_interrupted = None;
            }
            Err(interrupted) => snapshot.diagnostics_interrupted = Some(interrupted),
        },
    );
}

/// Espera a tarefa terminar e publica o resultado. Se ela entrar em pânico ou
/// for cancelada, publica a interrupção e a registra no log.
fn publish_when_done<T: Send + 'static>(
    task: JoinHandle<T>,
    operation: CoreOperation,
    state: Arc<watch::Sender<AppSnapshot>>,
    publish: impl FnOnce(&mut AppSnapshot, Result<T, TaskInterrupted>) + Send + 'static,
) {
    tokio::spawn(async move {
        let result = task.await.map_err(|join_error| {
            let interrupted = interrupted_by(operation, join_error);
            log_interruption(&state, &interrupted);
            interrupted
        });
        state.send_modify(|snapshot| publish(snapshot, result));
    });
}

/// Converte o fim inesperado de uma tarefa, guardando o erro original.
fn interrupted_by(operation: CoreOperation, join_error: JoinError) -> TaskInterrupted {
    let cause = if join_error.is_panic() {
        InterruptionCause::Panicked
    } else {
        InterruptionCause::Cancelled
    };
    TaskInterrupted {
        operation,
        cause,
        source: Some(Arc::new(join_error)),
    }
}

fn log_interruption(state: &watch::Sender<AppSnapshot>, interrupted: &TaskInterrupted) {
    let diagnostic = interrupted.to_diagnostic(&state.borrow().redactor);
    log_error(&diagnostic, "core task interrupted");
}

/// Grava os eventos em uma única tarefa, um de cada vez, para manter no
/// arquivo a mesma ordem de chegada. O laço termina quando o envio deixa de
/// existir (ver `send_to_recorder`) ou quando a gravação é interrompida.
fn spawn_recorder(
    recorder: Arc<dyn GsiRecorder>,
    state: Arc<watch::Sender<AppSnapshot>>,
) -> mpsc::UnboundedSender<RecordedPayload> {
    let (payload_sender, mut payload_receiver) = mpsc::unbounded_channel::<RecordedPayload>();
    tokio::spawn(async move {
        while let Some((received_at, raw)) = payload_receiver.recv().await {
            let recorder = Arc::clone(&recorder);
            match tokio::task::spawn_blocking(move || recorder.record(received_at, &raw)).await {
                Ok(result) => state.send_modify(|snapshot| {
                    if let Some(log_status) = &mut snapshot.gsi_log {
                        apply_record_result(log_status, result);
                    }
                }),
                Err(join_error) => {
                    stop_recording(&state, interrupted_by(CoreOperation::GsiLog, join_error));
                    break;
                }
            }
        }
    });
    payload_sender
}

/// Entrega o evento ao gravador. Canal fechado significa que a tarefa de
/// gravação parou: a gravação fica marcada como interrompida e o envio deixa
/// de ser tentado. O evento descartado não é registrado no log.
fn send_to_recorder(
    sender: &mut Option<mpsc::UnboundedSender<RecordedPayload>>,
    payload: RecordedPayload,
    state: &watch::Sender<AppSnapshot>,
) {
    let Some(active_sender) = sender else {
        return;
    };
    if active_sender.send(payload).is_err() {
        *sender = None;
        stop_recording(
            state,
            TaskInterrupted {
                operation: CoreOperation::GsiLog,
                cause: InterruptionCause::ChannelClosed,
                source: None,
            },
        );
    }
}

/// Marca a gravação como parada, mantendo o primeiro motivo conhecido.
fn stop_recording(state: &watch::Sender<AppSnapshot>, interrupted: TaskInterrupted) {
    let mut newly_stopped = false;
    state.send_modify(|snapshot| {
        if let Some(log_status) = &mut snapshot.gsi_log
            && log_status.stopped.is_none()
        {
            log_status.stopped = Some(interrupted.clone());
            newly_stopped = true;
        }
    });
    if newly_stopped {
        log_interruption(state, &interrupted);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/application/core_tests.rs"]
mod tests;
