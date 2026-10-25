//! Testes unitários de `src/application/core.rs`.

use super::*;
use crate::application::error::{DiagnosticCode, Redactor};
use crate::application::ports::{BoxFuture, ConnectionTestError, GsiRecordError, LocatorResult};
use crate::application::state::{GsiLogStatus, ModeOrigin, ModeState};
use crate::domain::game::{GameMode, GamePhase, GameSnapshot};
use crate::domain::match_events::{MatchEvent, MatchEventKind, Provenance};
use crate::test_support::{CapturedLogs, MemoryFiles, capture_logs};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize};

/// Tempo máximo de espera por uma mudança de estado.
const STATE_TIMEOUT: Duration = Duration::from_secs(5);
const SIMULATED_SECRET: &str = "segredo-simulado-123";

/// Localizador sem bibliotecas que entra em pânico enquanto `panics` for verdadeiro.
#[derive(Default)]
struct SwitchableLocator {
    panics: AtomicBool,
}

impl DotaLocator for SwitchableLocator {
    fn locate(&self) -> LocatorResult {
        assert!(
            !self.panics.load(Ordering::SeqCst),
            "localizador simulado falhou"
        );
        LocatorResult {
            searched_libraries: Vec::new(),
            installs: Vec::new(),
        }
    }
}

struct ClosedDota;
impl ProcessProbe for ClosedDota {
    fn dota_running(&self) -> bool {
        false
    }
}

/// Arquivos que entram em pânico em qualquer acesso.
struct PanickingFiles;
impl ConfigFiles for PanickingFiles {
    fn read(&self, _path: &Path) -> io::Result<Option<String>> {
        panic!("leitura simulada falhou")
    }
    fn ensure_dir(&self, _dir: &Path) -> io::Result<()> {
        panic!("criação de pasta simulada falhou")
    }
    fn write(&self, _path: &Path, _contents: &str) -> io::Result<()> {
        panic!("gravação simulada falhou")
    }
    fn backup(&self, _path: &Path) -> io::Result<PathBuf> {
        panic!("backup simulado falhou")
    }
}

/// Testador de conexão que entra em pânico ao enviar.
struct PanickingTester;
impl ConnectionTester for PanickingTester {
    fn send_test_event<'a>(
        &'a self,
        _endpoint: &'a GsiEndpoint,
    ) -> BoxFuture<'a, Result<(), ConnectionTestError>> {
        Box::pin(async { panic!("teste de conexão simulado falhou") })
    }
}

/// Gravador que entra em pânico com um segredo na mensagem, como se o
/// payload tivesse ido parar nela.
struct PanickingRecorder;
impl GsiRecorder for PanickingRecorder {
    fn record(&self, _received_at: SystemTime, _raw: &str) -> Result<(), GsiRecordError> {
        panic!("falha ao gravar {SIMULATED_SECRET}")
    }
}

fn dependencies() -> CoreDependencies {
    CoreDependencies {
        locator: Arc::new(SwitchableLocator::default()),
        probe: Arc::new(ClosedDota),
        files: Arc::new(MemoryFiles::default()),
        tester: Arc::new(PanickingTester),
        recorder: None,
        items: Arc::default(),
        endpoint: GsiEndpoint {
            port: 53000,
            token: "token".into(),
        },
    }
}

fn log_status() -> GsiLogStatus {
    GsiLogStatus {
        path: PathBuf::from("logs/gsi.jsonl"),
        written: 0,
        last_error: None,
        stopped: None,
    }
}

struct RunningCore {
    commands: mpsc::UnboundedSender<UiCommand>,
    events: mpsc::Sender<CoreEvent>,
    state: watch::Receiver<AppSnapshot>,
}

fn start(dependencies: CoreDependencies, initial: AppSnapshot) -> RunningCore {
    let (commands, command_receiver) = mpsc::unbounded_channel();
    let (events, event_receiver) = mpsc::channel(16);
    let (state_sender, state) = watch::channel(initial);
    tokio::spawn(run(
        dependencies,
        command_receiver,
        event_receiver,
        Arc::new(state_sender),
    ));
    RunningCore {
        commands,
        events,
        state,
    }
}

async fn wait_for(
    state: &mut watch::Receiver<AppSnapshot>,
    condition: impl FnMut(&AppSnapshot) -> bool,
) -> AppSnapshot {
    tokio::time::timeout(STATE_TIMEOUT, state.wait_for(condition))
        .await
        .expect("o estado esperado deve ser publicado a tempo")
        .expect("o núcleo deve continuar rodando")
        .clone()
}

fn dota_event() -> CoreEvent {
    CoreEvent::GsiAccepted {
        raw: format!(r#"{{"player":{{"name":"{SIMULATED_SECRET}"}}}}"#).into(),
        at: SystemTime::now(),
        is_test: false,
        game: Some(Box::new(GameSnapshot::menu())),
    }
}

fn dota_event_in(phase: GamePhase) -> CoreEvent {
    CoreEvent::GsiAccepted {
        raw: "{}".into(),
        at: SystemTime::now(),
        is_test: false,
        game: Some(Box::new(GameSnapshot {
            phase: Some(phase),
            ..GameSnapshot::menu()
        })),
    }
}

#[tokio::test]
async fn dota_events_update_game_state_and_match_timeline() {
    let mut core = start(dependencies(), AppSnapshot::default());

    core.events.send(dota_event()).await.unwrap();
    core.events
        .send(dota_event_in(GamePhase::HeroSelection))
        .await
        .unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        !snapshot.match_status.timeline.is_empty()
    })
    .await;
    assert_eq!(
        snapshot
            .match_status
            .game
            .as_ref()
            .and_then(|game| game.phase.clone()),
        Some(GamePhase::HeroSelection)
    );
    let kinds: Vec<_> = snapshot
        .match_status
        .timeline
        .iter()
        .map(|entry| entry.event.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![MatchEvent {
            kind: MatchEventKind::MatchFound,
            provenance: Provenance::Inferred,
        }]
    );
}

#[tokio::test]
async fn test_events_do_not_change_match_state() {
    let mut core = start(dependencies(), AppSnapshot::default());

    core.events
        .send(CoreEvent::GsiAccepted {
            raw: "{}".into(),
            at: SystemTime::now(),
            is_test: true,
            game: None,
        })
        .await
        .unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        snapshot.gsi.last_test_event.is_some()
    })
    .await;
    assert_eq!(snapshot.match_status, Default::default());
}

#[tokio::test]
async fn mark_match_found_command_adds_manual_event() {
    let mut core = start(dependencies(), AppSnapshot::default());

    core.commands.send(UiCommand::MarkMatchFound).unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        !snapshot.match_status.timeline.is_empty()
    })
    .await;
    assert_eq!(
        snapshot.match_status.timeline[0].event,
        MatchEvent::manual_match_found()
    );
}

#[tokio::test]
async fn diagnostics_panic_is_published_as_interruption() {
    let locator = Arc::new(SwitchableLocator::default());
    locator.panics.store(true, Ordering::SeqCst);
    let mut core = start(
        CoreDependencies {
            locator: locator.clone(),
            ..dependencies()
        },
        AppSnapshot::default(),
    );

    let snapshot = wait_for(&mut core.state, |snapshot| {
        snapshot.diagnostics_interrupted.is_some()
    })
    .await;

    let interrupted = snapshot.diagnostics_interrupted.unwrap();
    assert_eq!(interrupted.operation, CoreOperation::Diagnostics);
    assert_eq!(interrupted.cause, InterruptionCause::Panicked);
    let diagnostic = interrupted.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::CoreTaskInterrupted);
    assert_eq!(
        diagnostic.technical_details.as_deref(),
        Some("a tarefa de diagnóstico foi interrompida (pânico)")
    );
    assert!(snapshot.diagnostics.is_none());
}

#[tokio::test]
async fn successful_diagnostics_clears_previous_interruption() {
    let locator = Arc::new(SwitchableLocator::default());
    locator.panics.store(true, Ordering::SeqCst);
    let mut core = start(
        CoreDependencies {
            locator: locator.clone(),
            ..dependencies()
        },
        AppSnapshot::default(),
    );
    wait_for(&mut core.state, |snapshot| {
        snapshot.diagnostics_interrupted.is_some()
    })
    .await;

    locator.panics.store(false, Ordering::SeqCst);
    core.commands.send(UiCommand::RunDiagnostics).unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| snapshot.diagnostics.is_some()).await;
    assert!(snapshot.diagnostics_interrupted.is_none());
}

#[tokio::test]
async fn connection_test_panic_leaves_running_state_as_interruption() {
    let mut core = start(dependencies(), AppSnapshot::default());

    core.commands.send(UiCommand::TestConnection).unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        matches!(
            snapshot.connection_test,
            Some(ConnectionTest::Interrupted(_))
        )
    })
    .await;
    let Some(ConnectionTest::Interrupted(interrupted)) = snapshot.connection_test else {
        unreachable!("a condição acima garante Interrupted");
    };
    assert_eq!(interrupted.operation, CoreOperation::ConnectionTest);
    assert_eq!(interrupted.cause, InterruptionCause::Panicked);
}

#[tokio::test]
async fn install_panic_is_published_and_counts_as_install_attempt() {
    let mut core = start(
        CoreDependencies {
            files: Arc::new(PanickingFiles),
            ..dependencies()
        },
        AppSnapshot::default(),
    );

    core.commands
        .send(UiCommand::InstallGsiConfig {
            install_root: PathBuf::from("/lib"),
            overwrite: false,
        })
        .unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| snapshot.install_attempt == 1).await;
    let Some(InstallOutcome::Interrupted(interrupted)) = snapshot.last_install else {
        panic!(
            "esperava instalação interrompida: {:?}",
            snapshot.last_install
        );
    };
    assert_eq!(interrupted.operation, CoreOperation::InstallGsiConfig);
    assert_eq!(interrupted.cause, InterruptionCause::Panicked);
}

#[tokio::test]
async fn recorder_panic_stops_recording_without_exposing_its_message() {
    let (_guard, logs) = capture_logs();
    let mut core = start(
        CoreDependencies {
            recorder: Some(Arc::new(PanickingRecorder)),
            ..dependencies()
        },
        AppSnapshot {
            gsi_log: Some(log_status()),
            ..AppSnapshot::default()
        },
    );

    core.events.send(dota_event()).await.unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        snapshot
            .gsi_log
            .as_ref()
            .is_some_and(|log_status| log_status.stopped.is_some())
    })
    .await;
    let log_status = snapshot.gsi_log.unwrap();
    let stopped = log_status.stopped.unwrap();
    assert_eq!(stopped.operation, CoreOperation::GsiLog);
    assert_eq!(stopped.cause, InterruptionCause::Panicked);
    assert_eq!(log_status.written, 0);

    let diagnostic = stopped.to_diagnostic(&Redactor::default());
    assert_eq!(diagnostic.code, DiagnosticCode::CoreTaskInterrupted);
    assert!(!format!("{diagnostic:?}").contains(SIMULATED_SECRET));
    let log_text = logs.text();
    assert!(
        log_text.contains("code=CORE_TASK_INTERRUPTED"),
        "{log_text}"
    );
    assert!(!log_text.contains(SIMULATED_SECRET), "{log_text}");
}

#[tokio::test]
async fn events_after_recorder_stopped_keep_first_reason_and_are_still_counted() {
    let mut core = start(
        CoreDependencies {
            recorder: Some(Arc::new(PanickingRecorder)),
            ..dependencies()
        },
        AppSnapshot {
            gsi_log: Some(log_status()),
            ..AppSnapshot::default()
        },
    );
    core.events.send(dota_event()).await.unwrap();
    wait_for(&mut core.state, |snapshot| {
        snapshot
            .gsi_log
            .as_ref()
            .is_some_and(|log_status| log_status.stopped.is_some())
    })
    .await;

    core.events.send(dota_event()).await.unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| snapshot.gsi.dota_events == 2).await;
    let log_status = snapshot.gsi_log.unwrap();
    assert_eq!(
        log_status.stopped.map(|stopped| stopped.cause),
        Some(InterruptionCause::Panicked)
    );
    assert_eq!(log_status.written, 0);
}

#[test]
fn closed_recorder_channel_marks_recording_as_stopped_and_drops_sender() {
    let (state_sender, state) = watch::channel(AppSnapshot {
        gsi_log: Some(log_status()),
        ..AppSnapshot::default()
    });
    let (payload_sender, payload_receiver) = mpsc::unbounded_channel();
    drop(payload_receiver);
    let mut sender = Some(payload_sender);

    send_to_recorder(&mut sender, (SystemTime::now(), "{}".into()), &state_sender);

    assert!(sender.is_none());
    let stopped = state.borrow().gsi_log.as_ref().unwrap().stopped.clone();
    let stopped = stopped.expect("a gravação fica marcada como parada");
    assert_eq!(stopped.operation, CoreOperation::GsiLog);
    assert_eq!(stopped.cause, InterruptionCause::ChannelClosed);
    assert!(stopped.source.is_none());
}

#[test]
fn sending_without_recorder_does_nothing() {
    let (state_sender, state) = watch::channel(AppSnapshot {
        gsi_log: Some(log_status()),
        ..AppSnapshot::default()
    });
    let mut sender = None;

    send_to_recorder(&mut sender, (SystemTime::now(), "{}".into()), &state_sender);

    assert!(state.borrow().gsi_log.as_ref().unwrap().stopped.is_none());
}

/// Localizador que identifica cada chamada, em ordem, por
/// `searched_libraries = ["/rodada-<n>"]`. A chamada `gated_call` avisa que
/// começou e só termina quando o teste a libera; com `panics_after_release`,
/// ela entra em pânico depois de liberada.
struct GatedLocator {
    calls: AtomicUsize,
    gated_call: usize,
    panics_after_release: bool,
    started: mpsc::UnboundedSender<()>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

/// Controle da chamada presa do `GatedLocator`.
struct Gate {
    started: mpsc::UnboundedReceiver<()>,
    release: std::sync::mpsc::Sender<()>,
}

impl Gate {
    async fn wait_started(&mut self) {
        tokio::time::timeout(STATE_TIMEOUT, self.started.recv())
            .await
            .expect("a chamada presa deve começar a tempo")
            .expect("o localizador continua existindo");
    }

    fn release(&self) {
        self.release.send(()).unwrap();
    }
}

fn gated_locator(gated_call: usize, panics_after_release: bool) -> (Arc<GatedLocator>, Gate) {
    let (started_sender, started) = mpsc::unbounded_channel();
    let (release, release_receiver) = std::sync::mpsc::channel();
    let locator = GatedLocator {
        calls: AtomicUsize::new(0),
        gated_call,
        panics_after_release,
        started: started_sender,
        release: Mutex::new(release_receiver),
    };
    (Arc::new(locator), Gate { started, release })
}

impl DotaLocator for GatedLocator {
    fn locate(&self) -> LocatorResult {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.gated_call {
            self.started.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv()
                .expect("o teste libera a chamada presa");
            assert!(!self.panics_after_release, "rodada simulada falhou");
        }
        LocatorResult {
            searched_libraries: vec![PathBuf::from(format!("/rodada-{call}"))],
            installs: Vec::new(),
        }
    }
}

fn start_with_locator(locator: Arc<GatedLocator>) -> RunningCore {
    start(
        CoreDependencies {
            locator,
            ..dependencies()
        },
        AppSnapshot::default(),
    )
}

/// Chamada do localizador que gerou o diagnóstico exibido.
fn shown_round(snapshot: &AppSnapshot) -> Option<PathBuf> {
    snapshot
        .diagnostics
        .as_ref()
        .and_then(|report| report.searched_libraries.first().cloned())
}

fn round_path(call: usize) -> Option<PathBuf> {
    Some(PathBuf::from(format!("/rodada-{call}")))
}

const DISCARDED_ROUND_LOG: &str = "discarded diagnostics result from an older round";

/// Espera o descarte de uma rodada antiga, que não muda o estado e por isso
/// só aparece no log.
async fn wait_for_log(logs: &CapturedLogs, text: &str) {
    tokio::time::timeout(STATE_TIMEOUT, async {
        while !logs.text().contains(text) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("o log deve conter {text:?}: {}", logs.text()));
}

fn install_command() -> UiCommand {
    UiCommand::InstallGsiConfig {
        install_root: PathBuf::from("/lib"),
        overwrite: false,
    }
}

#[test]
fn diagnostics_rounds_are_numbered_in_order_and_only_the_last_is_latest() {
    let rounds = DiagnosticsRounds::default();
    let first = rounds.start();
    let second = rounds.start();

    assert_eq!((first, second), (1, 2));
    assert!(!rounds.is_latest(first));
    assert!(rounds.is_latest(second));
}

#[tokio::test]
async fn older_diagnostics_finishing_after_install_does_not_replace_install_diagnostics() {
    let (_guard, logs) = capture_logs();
    let (locator, mut gate) = gated_locator(1, false);
    let mut core = start_with_locator(locator);

    // 1. O diagnóstico automático (rodada 1) começa e fica preso.
    gate.wait_started().await;

    // 2. A instalação termina e publica o diagnóstico da rodada 2.
    core.commands.send(install_command()).unwrap();
    let snapshot = wait_for(&mut core.state, |snapshot| snapshot.install_attempt == 1).await;
    assert!(
        matches!(snapshot.last_install, Some(InstallOutcome::Created(_))),
        "{:?}",
        snapshot.last_install
    );
    assert_eq!(shown_round(&snapshot), round_path(2));

    // 3. O diagnóstico antigo termina depois.
    gate.release();
    wait_for_log(&logs, DISCARDED_ROUND_LOG).await;

    // 4. O diagnóstico novo permanece.
    let snapshot = core.state.borrow().clone();
    assert_eq!(shown_round(&snapshot), round_path(2));
    assert!(snapshot.diagnostics_interrupted.is_none());
    assert_eq!(snapshot.install_attempt, 1);
}

#[tokio::test]
async fn older_interruption_does_not_overwrite_newer_report() {
    let (_guard, logs) = capture_logs();
    let (locator, mut gate) = gated_locator(1, true);
    let mut core = start_with_locator(locator);
    gate.wait_started().await;

    core.commands.send(UiCommand::RunDiagnostics).unwrap();
    wait_for(&mut core.state, |snapshot| {
        shown_round(snapshot) == round_path(2)
    })
    .await;

    gate.release();
    wait_for_log(&logs, DISCARDED_ROUND_LOG).await;

    let snapshot = core.state.borrow().clone();
    assert!(
        snapshot.diagnostics_interrupted.is_none(),
        "{:?}",
        snapshot.diagnostics_interrupted
    );
    assert_eq!(shown_round(&snapshot), round_path(2));
}

#[tokio::test]
async fn install_superseded_by_newer_round_keeps_newer_report_and_still_reports_install() {
    let (_guard, logs) = capture_logs();
    let (locator, mut gate) = gated_locator(2, false);
    let mut core = start_with_locator(locator);
    wait_for(&mut core.state, |snapshot| {
        shown_round(snapshot) == round_path(1)
    })
    .await;

    // A rodada da instalação (chamada 2) fica presa.
    core.commands.send(install_command()).unwrap();
    gate.wait_started().await;
    // O diagnóstico manual começa depois e publica a chamada 3.
    core.commands.send(UiCommand::RunDiagnostics).unwrap();
    wait_for(&mut core.state, |snapshot| {
        shown_round(snapshot) == round_path(3)
    })
    .await;

    gate.release();
    let snapshot = wait_for(&mut core.state, |snapshot| snapshot.install_attempt == 1).await;

    assert!(
        matches!(snapshot.last_install, Some(InstallOutcome::Created(_))),
        "{:?}",
        snapshot.last_install
    );
    assert_eq!(shown_round(&snapshot), round_path(3));
    assert!(logs.text().contains(DISCARDED_ROUND_LOG), "{}", logs.text());
}

#[tokio::test]
async fn newer_round_replaces_report_of_an_older_round() {
    let (locator, _gate) = gated_locator(0, false);
    let mut core = start_with_locator(locator);
    wait_for(&mut core.state, |snapshot| {
        shown_round(snapshot) == round_path(1)
    })
    .await;

    core.commands.send(UiCommand::RunDiagnostics).unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        shown_round(snapshot) == round_path(2)
    })
    .await;
    assert!(snapshot.diagnostics_interrupted.is_none());
}

#[tokio::test]
async fn install_round_clears_interruption_of_an_older_round() {
    let (locator, mut gate) = gated_locator(1, true);
    let mut core = start_with_locator(locator);
    gate.wait_started().await;
    gate.release();
    let interrupted = wait_for(&mut core.state, |snapshot| {
        snapshot.diagnostics_interrupted.is_some()
    })
    .await;
    assert!(interrupted.diagnostics.is_none());

    core.commands.send(install_command()).unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| snapshot.install_attempt == 1).await;
    assert!(snapshot.diagnostics_interrupted.is_none());
    assert_eq!(shown_round(&snapshot), round_path(2));
}

#[tokio::test]
async fn chosen_game_mode_is_published_for_the_found_match() {
    let mut core = start(dependencies(), AppSnapshot::default());
    core.commands.send(UiCommand::MarkMatchFound).unwrap();
    wait_for(&mut core.state, |snapshot| {
        snapshot.match_status.match_number == 1
    })
    .await;

    core.commands
        .send(UiCommand::ChooseGameMode {
            match_number: 1,
            mode: GameMode::Turbo,
            origin: ModeOrigin::Chosen,
        })
        .unwrap();

    let snapshot = wait_for(&mut core.state, |snapshot| {
        matches!(snapshot.match_status.mode, ModeState::Confirmed { .. })
    })
    .await;
    assert_eq!(
        snapshot.match_status.mode,
        ModeState::Confirmed {
            mode: GameMode::Turbo,
            origin: ModeOrigin::Chosen
        }
    );
    assert_eq!(
        snapshot.match_status.last_confirmed_mode,
        Some(GameMode::Turbo)
    );
}

#[tokio::test]
async fn restarted_core_asks_with_all_pick_again() {
    let mut first_run = start(dependencies(), AppSnapshot::default());
    first_run.commands.send(UiCommand::MarkMatchFound).unwrap();
    wait_for(&mut first_run.state, |snapshot| {
        snapshot.match_status.match_number == 1
    })
    .await;
    first_run
        .commands
        .send(UiCommand::ChooseGameMode {
            match_number: 1,
            mode: GameMode::Bots,
            origin: ModeOrigin::Chosen,
        })
        .unwrap();
    wait_for(&mut first_run.state, |snapshot| {
        snapshot.match_status.last_confirmed_mode == Some(GameMode::Bots)
    })
    .await;

    let mut second_run = start(dependencies(), AppSnapshot::default());
    second_run.commands.send(UiCommand::MarkMatchFound).unwrap();

    let snapshot = wait_for(&mut second_run.state, |snapshot| {
        snapshot.match_status.match_number == 1
    })
    .await;
    assert_eq!(
        snapshot.match_status.mode,
        ModeState::Awaiting {
            preselected: GameMode::AllPick
        }
    );
}
