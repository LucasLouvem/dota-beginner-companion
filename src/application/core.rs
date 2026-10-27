//! Laço principal: recebe eventos dos adaptadores e comandos da interface,
//! executa os casos de uso e publica o novo `AppSnapshot`.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use tokio::sync::{mpsc, watch};
use tokio::task::{JoinError, JoinHandle};

use crate::domain::gsi_config::GsiEndpoint;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_events::MatchTracker;
use crate::domain::patch::Patch;

use super::diagnostics::DiagnosticReport;
use super::error::{ToDiagnostic, log_error};
use super::event_bus::{CoreEvent, UiCommand};
use super::ports::{ConfigFiles, ConnectionTester, DotaLocator, GsiRecorder, ProcessProbe};
use super::state::{
    AppSnapshot, ConnectionTest, CoreOperation, InstallOutcome, InterruptionCause,
    RecommendationStatus, TaskInterrupted,
};
use super::use_cases::current_patch::load_current_patch;
use super::use_cases::external_sources::ExternalSources;
use super::use_cases::install_gsi_config::install_gsi_config;
use super::use_cases::item_icons::{
    IconBytes, IconSources, carried_items, load_icons, status_hero_images, status_items,
};
use super::use_cases::recommendation_request::{RequestTicket, RequestTracker};
use super::use_cases::recommendations::{Recommendations, desired_key, load_recommendations};
use super::use_cases::record_gsi_event::record_gsi_event;
use super::use_cases::record_gsi_payload::{apply_record_result, payload_to_record};
use super::use_cases::run_diagnostics::run_diagnostics;
use super::use_cases::track_match::{
    record_manual_match_found, record_match_update, record_mode_choice,
};

const DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(10);

/// Evento do Dota a gravar no log: horário de chegada e JSON sem `auth`.
type RecordedPayload = (SystemTime, Arc<str>);

/// Numeração das rodadas de diagnóstico: automática, manual e a feita depois
/// da instalação seguem a mesma sequência. As tarefas rodam em paralelo e
/// podem terminar fora de ordem; só a rodada iniciada por último publica o
/// resultado, para que um resultado antigo (inclusive uma interrupção) nunca
/// apareça por cima de um mais novo.
///
/// O `Arc` permite que o laço do núcleo, que numera as rodadas, e as tarefas,
/// que conferem a numeração ao publicar, compartilhem o mesmo contador.
#[derive(Debug, Clone, Default)]
struct DiagnosticsRounds(Arc<AtomicU64>);

impl DiagnosticsRounds {
    /// Inicia uma rodada e devolve o número dela.
    fn start(&self) -> u64 {
        // `AcqRel`/`Acquire`: quem confere em `is_latest` vê o número mais
        // recente gravado aqui.
        self.0.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Indica se nenhuma rodada começou depois desta.
    fn is_latest(&self, round: u64) -> bool {
        self.0.load(Ordering::Acquire) == round
    }
}

/// Registra que o resultado de uma rodada foi descartado por haver outra mais nova.
fn log_discarded_round(operation: CoreOperation, round: u64) {
    tracing::debug!(
        ?operation,
        round,
        "discarded diagnostics result from an older round"
    );
}

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
    /// Fontes das recomendações; ausentes quando o banco local ou o cliente
    /// HTTP não puderam ser abertos.
    pub external: Option<ExternalSources>,
    /// Imagens dos itens; ausentes quando a pasta de cache ou o cliente HTTP
    /// não estão disponíveis.
    pub icons: Option<IconSources>,
}

/// Imagens pedidas e ainda não publicadas. Cada item é pedido uma única vez
/// por execução: uma falha não é repetida até a próxima abertura.
struct IconRuntime {
    sources: IconSources,
    requested: HashSet<String>,
    /// Recomendações cujos itens já foram pedidos, para não percorrer as
    /// builds a cada evento do GSI.
    seen_recommendations: Option<Arc<Recommendations>>,
    sender: mpsc::UnboundedSender<Vec<(String, IconBytes)>>,
}

impl IconRuntime {
    fn new(sources: IconSources, sender: mpsc::UnboundedSender<Vec<(String, IconBytes)>>) -> Self {
        Self {
            sources,
            requested: HashSet::new(),
            seen_recommendations: None,
            sender,
        }
    }

    /// Pede as imagens novas: as do inventário atual, as dos heróis e das
    /// habilidades que a página mostra e, quando as recomendações mudaram, as
    /// dos itens das builds.
    fn refresh(&mut self, snapshot: &AppSnapshot) {
        let mut wanted: BTreeSet<String> = snapshot
            .match_status
            .game
            .as_ref()
            .map(carried_items)
            .unwrap_or_default();
        wanted.extend(status_hero_images(snapshot, &self.sources.heroes));
        if let RecommendationStatus::Ready(recommendations) = &snapshot.recommendations {
            let changed = !self
                .seen_recommendations
                .as_ref()
                .is_some_and(|seen| Arc::ptr_eq(seen, recommendations));
            if changed {
                self.seen_recommendations = Some(Arc::clone(recommendations));
                wanted.extend(status_items(&snapshot.recommendations));
            }
        }
        let missing: Vec<String> = wanted
            .into_iter()
            .filter(|item| self.requested.insert(item.clone()))
            .collect();
        if missing.is_empty() {
            return;
        }
        let sources = self.sources.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let loaded = load_icons(&sources, missing).await;
            if !loaded.is_empty() {
                // Canal fechado só no encerramento do runtime; nada a fazer.
                let _ = sender.send(loaded);
            }
        });
    }
}

/// Junta as imagens carregadas ao estado, num mapa novo: o `Arc` antigo
/// continua válido para quem já o leu.
fn publish_icons(state: &watch::Sender<AppSnapshot>, loaded: Vec<(String, IconBytes)>) {
    state.send_modify(|snapshot| {
        let mut icons = (*snapshot.item_icons).clone();
        icons.extend(loaded);
        snapshot.item_icons = Arc::new(icons);
    });
}

/// Resultado das tarefas de recomendação, entregue ao laço do núcleo.
enum RecommendationMessage {
    PatchLoaded(Option<Patch>),
    Loaded(RequestTicket, Box<Recommendations>),
    Interrupted(RequestTicket, TaskInterrupted),
}

/// Consultas de recomendação em andamento. Cada mudança de herói, modo ou
/// patch inicia uma consulta; só a resposta da consulta atual é publicada.
struct RecommendationRuntime {
    sources: ExternalSources,
    tracker: RequestTracker,
    patch: Option<Patch>,
    sender: mpsc::UnboundedSender<RecommendationMessage>,
}

impl RecommendationRuntime {
    /// Marca as recomendações como ociosas e começa a carregar o patch atual.
    fn start(
        sources: ExternalSources,
        sender: mpsc::UnboundedSender<RecommendationMessage>,
        state: &watch::Sender<AppSnapshot>,
    ) -> Self {
        state.send_modify(|snapshot| snapshot.recommendations = RecommendationStatus::Idle);
        let patch_sources = sources.clone();
        let patch_sender = sender.clone();
        tokio::spawn(async move {
            let patch = load_current_patch(
                patch_sources.provider,
                patch_sources.budget,
                patch_sources.cache,
                SystemTime::now(),
                patch_sources.embedded_patches,
            )
            .await;
            // Canal fechado só no encerramento do runtime; nada a fazer.
            let _ = patch_sender.send(RecommendationMessage::PatchLoaded(patch.value));
        });
        Self {
            sources,
            tracker: RequestTracker::default(),
            patch: None,
            sender,
        }
    }

    /// Inicia a consulta da chave desejada pelo estado atual, se ela mudou.
    fn refresh(&mut self, state: &watch::Sender<AppSnapshot>) {
        let wanted = desired_key(
            &state.borrow().match_status,
            self.patch.as_ref().map(|patch| patch.id),
        );
        let Some(key) = wanted else {
            if self.tracker.current().is_some() {
                self.tracker.clear();
                state.send_modify(|snapshot| snapshot.recommendations = RecommendationStatus::Idle);
            }
            return;
        };
        let (Some(ticket), Some(patch)) = (self.tracker.start(key.clone()), self.patch.clone())
        else {
            return;
        };
        state.send_modify(|snapshot| {
            snapshot.recommendations = RecommendationStatus::Loading { key: key.clone() }
        });
        let sources = self.sources.clone();
        let sender = self.sender.clone();
        let task = tokio::spawn(async move {
            load_recommendations(&sources, key, patch, SystemTime::now()).await
        });
        tokio::spawn(async move {
            let message = match task.await {
                Ok(recommendations) => {
                    RecommendationMessage::Loaded(ticket, Box::new(recommendations))
                }
                Err(join_error) => RecommendationMessage::Interrupted(
                    ticket,
                    interrupted_by(CoreOperation::Recommendations, join_error),
                ),
            };
            // Canal fechado só no encerramento do runtime; nada a fazer.
            let _ = sender.send(message);
        });
    }

    fn handle(&mut self, message: RecommendationMessage, state: &watch::Sender<AppSnapshot>) {
        match message {
            RecommendationMessage::PatchLoaded(Some(patch)) => {
                self.patch = Some(patch);
                self.refresh(state);
            }
            RecommendationMessage::PatchLoaded(None) => {
                tracing::warn!("current patch unknown; recommendations unavailable");
            }
            RecommendationMessage::Loaded(ticket, recommendations) => {
                if self.tracker.accepts(&ticket) {
                    state.send_modify(|snapshot| {
                        snapshot.recommendations =
                            RecommendationStatus::Ready(Arc::from(recommendations))
                    });
                } else {
                    tracing::debug!(
                        generation = ticket.generation,
                        "discarded recommendations of an older request"
                    );
                }
            }
            RecommendationMessage::Interrupted(ticket, interrupted) => {
                log_interruption(state, &interrupted);
                if self.tracker.accepts(&ticket) {
                    state.send_modify(|snapshot| {
                        snapshot.recommendations = RecommendationStatus::Interrupted {
                            key: ticket.key,
                            interrupted,
                        }
                    });
                }
            }
        }
    }
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
    let rounds = DiagnosticsRounds::default();
    let (recommendation_sender, mut recommendation_receiver) = mpsc::unbounded_channel();
    let mut recommendations = dependencies
        .external
        .clone()
        .map(|sources| RecommendationRuntime::start(sources, recommendation_sender, &state));
    let (icon_sender, mut icon_receiver) = mpsc::unbounded_channel();
    let mut icons = dependencies
        .icons
        .clone()
        .map(|sources| IconRuntime::new(sources, icon_sender));
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
                if let Some(runtime) = &mut recommendations {
                    runtime.refresh(&state);
                }
                if let Some(runtime) = &mut icons {
                    runtime.refresh(&state.borrow());
                }
            }
            Some(command) = commands.recv() => {
                handle_command(&dependencies, &state, &rounds, command);
                if let Some(runtime) = &mut recommendations {
                    runtime.refresh(&state);
                }
            }
            Some(message) = recommendation_receiver.recv() => {
                if let Some(runtime) = &mut recommendations {
                    runtime.handle(message, &state);
                }
                if let Some(runtime) = &mut icons {
                    runtime.refresh(&state.borrow());
                }
            }
            Some(loaded) = icon_receiver.recv() => publish_icons(&state, loaded),
            _ = ticker.tick() => spawn_diagnostics(&dependencies, &state, &rounds),
        }
    }
}

fn handle_command(
    dependencies: &CoreDependencies,
    state: &Arc<watch::Sender<AppSnapshot>>,
    rounds: &DiagnosticsRounds,
    command: UiCommand,
) {
    match command {
        UiCommand::RunDiagnostics => spawn_diagnostics(dependencies, state, rounds),
        UiCommand::MarkMatchFound => state.send_modify(|snapshot| {
            record_manual_match_found(&mut snapshot.match_status, SystemTime::now());
        }),
        UiCommand::ChooseGameMode {
            match_number,
            mode,
            origin,
        } => state.send_modify(|snapshot| {
            record_mode_choice(&mut snapshot.match_status, match_number, mode, origin);
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
            let rounds = rounds.clone();
            let round = rounds.start();
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
                move |snapshot, result| {
                    match result {
                        Ok((outcome, report)) => {
                            snapshot.last_install = Some(outcome);
                            // O resultado da instalação sempre é publicado;
                            // o diagnóstico, só se nenhuma rodada começou depois.
                            if rounds.is_latest(round) {
                                snapshot.diagnostics = Some(report);
                                snapshot.diagnostics_interrupted = None;
                            } else {
                                log_discarded_round(CoreOperation::InstallGsiConfig, round);
                            }
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

/// Inicia uma rodada de diagnóstico. O resultado, inclusive uma interrupção,
/// só é publicado se nenhuma rodada começou depois desta.
fn spawn_diagnostics(
    dependencies: &CoreDependencies,
    state: &Arc<watch::Sender<AppSnapshot>>,
    rounds: &DiagnosticsRounds,
) {
    let dependencies = dependencies.clone();
    let rounds = rounds.clone();
    let round = rounds.start();
    let task = tokio::task::spawn_blocking(move || build_diagnostics(&dependencies));
    publish_when_done(
        task,
        CoreOperation::Diagnostics,
        Arc::clone(state),
        move |snapshot, result| {
            // A conferência roda dentro de `send_modify`, junto com a gravação.
            if !rounds.is_latest(round) {
                log_discarded_round(CoreOperation::Diagnostics, round);
                return;
            }
            match result {
                Ok(report) => {
                    snapshot.diagnostics = Some(report);
                    snapshot.diagnostics_interrupted = None;
                }
                Err(interrupted) => snapshot.diagnostics_interrupted = Some(interrupted),
            }
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
