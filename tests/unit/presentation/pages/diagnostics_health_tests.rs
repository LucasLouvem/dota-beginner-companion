//! Testes unitários de `src/presentation/pages/diagnostics_health.rs` (o
//! conteúdo da tela de saúde, sem widgets).

use super::*;
use crate::application::diagnostics::GsiConfigFileError;
use crate::application::event_bus::{CoreConnectionError, GsiServerError};
use crate::application::ports::ConnectionTestError;
use crate::application::state::{CoreOperation, InterruptionCause, TaskInterrupted};
use crate::domain::dota_install::DotaInstall;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

fn install_check(config: GsiConfigState) -> InstallCheck {
    let library = PathBuf::from("/steam");
    InstallCheck {
        install: DotaInstall {
            game_root: library.join("steamapps/common/dota 2 beta"),
            library,
            native_binary: true,
            proton_prefix: false,
        },
        config_path: PathBuf::from("/steam/gamestate_integration_dota_companion.cfg"),
        config,
    }
}

fn report(config: GsiConfigState, dota_running: bool) -> DiagnosticReport {
    DiagnosticReport {
        generated_at: UNIX_EPOCH,
        searched_libraries: vec![PathBuf::from("/steam")],
        installs: vec![install_check(config)],
        dota_process_running: dota_running,
        steam_process_running: true,
    }
}

/// Tudo pronto: servidor ouvindo, configuração válida e Dota fechado.
fn ready() -> AppSnapshot {
    let mut snapshot = AppSnapshot::default();
    snapshot.gsi.server = ServerState::Listening(SocketAddr::from(([127, 0, 0, 1], 53000)));
    snapshot.diagnostics = Some(report(GsiConfigState::Matches, false));
    snapshot.config_path = PathBuf::from("/config/config.toml");
    snapshot
}

fn event_at(at: SystemTime) -> ReceivedEvent {
    ReceivedEvent {
        at,
        raw: Arc::from(r#"{"player":{"name":"Fulano","steamid":"7656"}}"#),
    }
}

fn recent_event() -> ReceivedEvent {
    event_at(SystemTime::now())
}

fn old_event() -> ReceivedEvent {
    event_at(SystemTime::now() - Duration::from_secs(3600))
}

fn content(snapshot: &AppSnapshot) -> HealthContent {
    health_content(snapshot, None)
}

fn card<'a>(content: &'a HealthContent, title: &str) -> &'a HealthCard {
    content
        .cards
        .iter()
        .find(|card| card.title == title)
        .unwrap_or_else(|| panic!("sem card {title}"))
}

fn state_of(snapshot: &AppSnapshot, title: &str) -> String {
    card(&content(snapshot), title).state.clone()
}

fn interrupted() -> TaskInterrupted {
    TaskInterrupted {
        operation: CoreOperation::ConnectionTest,
        cause: InterruptionCause::Panicked,
        source: None,
    }
}

#[test]
fn the_screen_has_five_cards_in_reading_order() {
    let titles: Vec<String> = content(&ready())
        .cards
        .into_iter()
        .map(|card| card.title)
        .collect();
    assert_eq!(
        titles,
        [
            "Steam",
            "Dota 2",
            "Servidor GSI",
            "Configuração GSI",
            "Recebimento de dados"
        ]
    );
}

#[test]
fn steam_is_unknown_until_the_first_report() {
    let steam = card(&content(&AppSnapshot::default()), "Steam").clone();
    assert_eq!(steam.state, "Estado desconhecido");
    assert_eq!(steam.level, Level::Info);
}

#[test]
fn steam_is_open_or_not_detected_by_the_report() {
    let mut snapshot = ready();
    assert_eq!(state_of(&snapshot, "Steam"), "Aberta");

    let mut closed = report(GsiConfigState::Matches, false);
    closed.steam_process_running = false;
    snapshot.diagnostics = Some(closed);
    assert_eq!(state_of(&snapshot, "Steam"), "Não detectada");
}

#[test]
fn steam_not_detected_never_lowers_the_overall_state() {
    let mut snapshot = ready();
    let mut closed = report(GsiConfigState::Matches, false);
    closed.steam_process_running = false;
    snapshot.diagnostics = Some(closed);
    assert_eq!(content(&snapshot).overall, OverallState::Working);
}

#[test]
fn dota_is_running_not_detected_or_not_installed() {
    let mut snapshot = ready();
    assert_eq!(state_of(&snapshot, "Dota 2"), "Não detectado");

    snapshot.diagnostics = Some(report(GsiConfigState::Matches, true));
    assert_eq!(state_of(&snapshot, "Dota 2"), "Em execução");

    let mut none = report(GsiConfigState::Matches, false);
    none.installs.clear();
    snapshot.diagnostics = Some(none);
    let dota = card(&content(&snapshot), "Dota 2").clone();
    assert_eq!(dota.state, "Instalação não encontrada");
    assert_eq!(dota.level, Level::Error);
    assert!(
        !dota.description.contains("/steam"),
        "sem caminhos técnicos"
    );
}

#[test]
fn gsi_server_is_starting_working_or_failed() {
    let mut snapshot = ready();
    snapshot.gsi.server = ServerState::Starting;
    assert_eq!(state_of(&snapshot, "Servidor GSI"), "Iniciando");

    snapshot.gsi.server = ServerState::Listening(SocketAddr::from(([127, 0, 0, 1], 1)));
    let working = content(&snapshot);
    assert_eq!(card(&working, "Servidor GSI").state, "Funcionando");
    assert!(
        !card(&working, "Servidor GSI")
            .description
            .contains("127.0.0.1")
    );

    snapshot.gsi.server = ServerState::Failed(GsiServerError::Bind {
        address: SocketAddr::from(([127, 0, 0, 1], 53000)),
        source: Arc::new(io::Error::from(io::ErrorKind::AddrInUse)),
    });
    let failed = content(&snapshot);
    assert_eq!(card(&failed, "Servidor GSI").state, "Com erro");
    assert_eq!(card(&failed, "Servidor GSI").level, Level::Error);
    assert!(failed.results.iter().any(|row| row.level == Level::Error));
}

#[test]
fn gsi_config_states_read_in_plain_words() {
    let cases = [
        (GsiConfigState::Matches, "Instalada e válida", Level::Ok),
        (GsiConfigState::Missing, "Ausente", Level::Warning),
        (
            GsiConfigState::UriMismatch {
                found: Some("http://localhost:1/".into()),
            },
            "Aponta para outro endereço ou porta",
            Level::Warning,
        ),
        (
            GsiConfigState::TokenMismatch,
            "Token divergente",
            Level::Warning,
        ),
    ];
    for (config, state, level) in cases {
        let mut snapshot = ready();
        snapshot.diagnostics = Some(report(config, false));
        let config_card = card(&content(&snapshot), "Configuração GSI").clone();
        assert_eq!(config_card.state, state);
        assert_eq!(config_card.level, level);
        assert!(
            !config_card.description.contains("http://"),
            "sem endereço técnico"
        );
    }
}

#[test]
fn invalid_config_is_an_error_with_a_plain_resolution() {
    let mut snapshot = ready();
    snapshot.diagnostics = Some(report(
        GsiConfigState::Failed(GsiConfigFileError::Read {
            path: PathBuf::from("/steam/x.cfg"),
            source: Arc::new(io::Error::from(io::ErrorKind::PermissionDenied)),
        }),
        false,
    ));

    let health = content(&snapshot);

    let config = card(&health, "Configuração GSI");
    assert_eq!(config.state, "Inválida");
    assert_eq!(config.level, Level::Error);
    assert!(!config.description.is_empty());
    assert_eq!(health.overall, OverallState::NotReady);
}

#[test]
fn data_flow_is_receiving_waiting_stale_or_unstable() {
    let mut snapshot = ready();
    assert_eq!(
        state_of(&snapshot, "Recebimento de dados"),
        "Aguardando o Dota"
    );
    assert_eq!(data_flow(&snapshot, false), DataFlow::Waiting);

    snapshot.gsi.last_dota_event = Some(recent_event());
    assert_eq!(
        state_of(&snapshot, "Recebimento de dados"),
        "Recebendo dados"
    );

    snapshot.gsi.last_dota_event = Some(old_event());
    assert_eq!(
        state_of(&snapshot, "Recebimento de dados"),
        "Sem dados recentes"
    );

    snapshot.diagnostics = Some(report(GsiConfigState::Matches, true));
    assert_eq!(
        state_of(&snapshot, "Recebimento de dados"),
        "Conexão instável ou interrompida",
        "dados parados com o Dota aberto"
    );
}

#[test]
fn rejected_requests_after_the_last_event_make_the_connection_unstable() {
    let mut snapshot = ready();
    let event = recent_event();
    snapshot.gsi.last_rejection = Some((
        event.at + Duration::from_secs(1),
        RejectReason::InvalidToken,
    ));
    snapshot.gsi.last_dota_event = Some(event);
    assert_eq!(data_flow(&snapshot, false), DataFlow::Unstable);

    let mut only_rejections = ready();
    only_rejections.gsi.last_rejection = Some((SystemTime::now(), RejectReason::InvalidJson));
    assert_eq!(data_flow(&only_rejections, false), DataFlow::Unstable);
}

#[test]
fn waiting_with_the_game_open_asks_to_check_the_configuration() {
    let mut snapshot = ready();
    snapshot.diagnostics = Some(report(GsiConfigState::Matches, true));
    let data = card(&content(&snapshot), "Recebimento de dados").clone();
    assert_eq!(data.level, Level::Warning);
    assert!(data.description.contains("-gamestateintegration"));
}

#[test]
fn overall_state_follows_the_documented_order() {
    let mut snapshot = AppSnapshot::default();
    assert_eq!(content(&snapshot).overall, OverallState::Checking);

    snapshot = ready();
    assert_eq!(content(&snapshot).overall, OverallState::Working);

    snapshot.diagnostics = Some(report(GsiConfigState::Missing, false));
    assert_eq!(content(&snapshot).overall, OverallState::NotReady);

    snapshot = ready();
    snapshot.gsi.last_dota_event = Some(old_event());
    snapshot.diagnostics = Some(report(GsiConfigState::Matches, true));
    assert_eq!(content(&snapshot).overall, OverallState::Attention);

    let failure = CoreConnectionError::StateClosed.to_diagnostic(&Redactor::default());
    assert_eq!(
        health_content(&ready(), Some(&failure)).overall,
        OverallState::CoreUnavailable,
        "o núcleo indisponível vem antes de tudo"
    );
}

#[test]
fn closed_dota_with_no_data_is_still_working() {
    let health = content(&ready());
    assert_eq!(health.overall, OverallState::Working);
    assert_eq!(
        card(&health, "Recebimento de dados").state,
        "Aguardando o Dota"
    );
}

#[test]
fn core_failure_is_the_first_result() {
    let failure = CoreConnectionError::StateClosed.to_diagnostic(&Redactor::default());
    let health = health_content(&ready(), Some(&failure));
    assert_eq!(health.results[0], RowContent::diagnostic(&failure));
}

#[test]
fn connection_test_result_is_shown_without_claiming_the_dota_sends_data() {
    let mut snapshot = ready();
    snapshot.connection_test = Some(ConnectionTest::Passed);
    let passed = content(&snapshot);
    assert_eq!(passed.results[0].title, "Teste de conexão aprovado");
    assert!(passed.results[0].subtitle.contains("não confirma"));
    assert_eq!(passed.overall, OverallState::Working);

    snapshot.connection_test = Some(ConnectionTest::Failed(ConnectionTestError::TokenRejected));
    let refused = content(&snapshot);
    assert_eq!(refused.results[0].level, Level::Error);
    assert_eq!(refused.overall, OverallState::Attention);

    snapshot.connection_test = Some(ConnectionTest::Interrupted(interrupted()));
    assert_eq!(content(&snapshot).overall, OverallState::Attention);

    snapshot.connection_test = Some(ConnectionTest::Running);
    assert_eq!(content(&snapshot).results[0].title, "Testando a conexão…");
}

#[test]
fn install_results_do_not_show_the_file_path_on_success() {
    let mut snapshot = ready();
    snapshot.last_install = Some(InstallOutcome::Created(PathBuf::from("/steam/gsi.cfg")));
    let created = content(&snapshot).results.remove(0);
    assert_eq!(created.title, "Configuração GSI instalada");
    assert!(!created.subtitle.contains("/steam"));

    snapshot.last_install = Some(InstallOutcome::NeedsConfirmation(PathBuf::from(
        "/steam/gsi.cfg",
    )));
    let needs = content(&snapshot).results.remove(0);
    assert!(
        needs.subtitle.contains("/steam/gsi.cfg"),
        "o caminho aparece quando serve para decidir"
    );
}

#[test]
fn install_is_enabled_only_with_an_install_found() {
    assert!(content(&ready()).can_install);
    let mut none = ready();
    let mut empty = report(GsiConfigState::Matches, false);
    empty.installs.clear();
    none.diagnostics = Some(empty);
    assert!(!content(&none).can_install);
    assert!(!content(&AppSnapshot::default()).can_install);
}

/// Texto de tudo o que a tela principal mostra.
fn visible_text(health: &HealthContent) -> String {
    let mut parts = vec![health.overall.title(), health.overall.description()];
    for card in &health.cards {
        parts.extend([
            card.title.clone(),
            card.state.clone(),
            card.description.clone(),
        ]);
    }
    for row in &health.results {
        parts.extend([row.title.clone(), row.subtitle.clone()]);
    }
    parts.join("\n")
}

#[test]
fn the_main_screen_never_shows_token_payload_counters_or_recording() {
    let mut snapshot = ready();
    snapshot.gsi.last_dota_event = Some(recent_event());
    snapshot.gsi.dota_events = 1482;
    snapshot.gsi_log = Some(GsiLogStatus {
        path: PathBuf::from("logs/gsi.jsonl"),
        written: 1460,
        last_error: None,
        stopped: None,
    });
    snapshot.connection_test = Some(ConnectionTest::Passed);

    let text = visible_text(&content(&snapshot));

    for forbidden in [
        "1482",
        "1460",
        "gsi.jsonl",
        "Gravando",
        "Gravação",
        "JSON",
        "Fulano",
        "7656",
        "token",
        "config.toml",
        "player",
    ] {
        assert!(
            !text.contains(forbidden),
            "{forbidden:?} apareceu na tela:\n{text}"
        );
    }
}

#[test]
fn technical_details_keep_counters_recording_and_the_source_of_the_json() {
    let mut snapshot = ready();
    snapshot.gsi.last_dota_event = Some(recent_event());
    snapshot.gsi.dota_events = 1482;
    snapshot.gsi_log = Some(GsiLogStatus {
        path: PathBuf::from("logs/gsi.jsonl"),
        written: 1460,
        last_error: None,
        stopped: None,
    });
    snapshot.gsi.rejected = 2;
    snapshot.gsi.last_rejection = Some((UNIX_EPOCH, RejectReason::InvalidToken));

    let details = content(&snapshot).details;

    assert_eq!(details.received, "Eventos recebidos do Dota: 1482");
    assert_eq!(details.recorded.as_deref(), Some("Eventos gravados: 1460"));
    assert!(details.rejected.unwrap().contains("token inválido"));
    assert!(
        details.log.unwrap().subtitle.contains("não compartilhe"),
        "o aviso sobre SteamID e nome continua ao lado da gravação"
    );
    assert!(details.config_path.contains("/config/config.toml"));
    assert!(details.json_source.starts_with("Origem: Dota 2"));
}

#[test]
fn internal_state_is_untouched_by_building_the_content() {
    let mut snapshot = ready();
    snapshot.gsi.last_dota_event = Some(recent_event());
    snapshot.gsi.dota_events = 7;
    let before = snapshot.gsi.dota_events;
    let raw = snapshot
        .gsi
        .last_dota_event
        .as_ref()
        .map(|event| Arc::clone(&event.raw));

    let _ = content(&snapshot);

    assert_eq!(snapshot.gsi.dota_events, before);
    assert_eq!(
        snapshot
            .gsi
            .last_dota_event
            .as_ref()
            .map(|event| Arc::clone(&event.raw)),
        raw,
        "o JSON recebido continua guardado no estado"
    );
}

#[test]
fn the_same_state_gives_the_same_content_so_only_changed_cards_update() {
    let mut snapshot = ready();
    snapshot.gsi.last_dota_event = Some(recent_event());
    let first = content(&snapshot);
    assert_eq!(first, content(&snapshot.clone()));

    snapshot.diagnostics = Some(report(GsiConfigState::Missing, false));
    let second = content(&snapshot);
    let changed: Vec<usize> = first
        .cards
        .iter()
        .zip(&second.cards)
        .enumerate()
        .filter(|(_, (before, after))| before != after)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(changed, [3], "só o card da configuração muda");
}
