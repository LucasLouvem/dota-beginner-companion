//! Testes unitários de `src/presentation/pages/diagnostics.rs` (conteúdo da
//! página, sem widgets).

use super::*;
use crate::domain::dota_install::DotaInstall;
use crate::presentation::widgets::{RowChange, row_changes};
use std::net::SocketAddr;
use std::time::UNIX_EPOCH;

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

fn report(installs: Vec<InstallCheck>) -> DiagnosticReport {
    DiagnosticReport {
        generated_at: UNIX_EPOCH,
        searched_libraries: vec![PathBuf::from("/steam")],
        installs,
        dota_process_running: false,
    }
}

/// Estado com o servidor ouvindo, a instalação encontrada e o log de eventos
/// ativo, sem eventos recebidos (cujo horário relativo muda com o tempo).
fn logging_snapshot(written: u64) -> AppSnapshot {
    let mut snapshot = AppSnapshot::default();
    snapshot.gsi.server = ServerState::Listening(SocketAddr::from(([127, 0, 0, 1], 53000)));
    snapshot.diagnostics = Some(report(vec![install_check(GsiConfigState::Matches)]));
    snapshot.config_path = PathBuf::from("/config/config.toml");
    snapshot.gsi_log = Some(GsiLogStatus {
        path: PathBuf::from("logs/gsi.jsonl"),
        written,
        last_error: None,
        stopped: None,
    });
    snapshot
}

#[test]
fn only_the_log_row_is_updated_when_another_event_is_written() {
    let before = diagnostics_content(&logging_snapshot(10), None);
    let after = diagnostics_content(&logging_snapshot(11), None);

    assert_eq!(
        after.server[2],
        RowContent::new(
            Level::Info,
            "Gravando eventos do Dota: 11",
            "logs/gsi.jsonl — o arquivo contém SteamID e nome do jogador; não compartilhe.",
        )
    );
    assert_eq!(
        row_changes(&before.server, &after.server),
        vec![RowChange::Update(2)]
    );
    assert_eq!(row_changes(&before.dota, &after.dota), vec![]);
    assert_eq!(before.config_subtitle, after.config_subtitle);
    assert_eq!(before.json_source, after.json_source);
}

#[test]
fn snapshot_without_change_produces_the_same_content() {
    let snapshot = logging_snapshot(10);
    let shown = diagnostics_content(&snapshot, None);
    let same = diagnostics_content(&snapshot.clone(), None);

    assert_eq!(shown, same);
    assert_eq!(row_changes(&shown.dota, &same.dota), vec![]);
    assert_eq!(row_changes(&shown.server, &same.server), vec![]);
}

#[test]
fn found_install_enables_install_and_shows_game_and_config_rows() {
    let content = diagnostics_content(&logging_snapshot(0), None);

    assert!(content.can_install);
    assert_eq!(
        content.dota,
        vec![
            RowContent::new(
                Level::Info,
                "Dota 2 não está aberto",
                "Nenhum processo dota2 encontrado. Normal se o jogo estiver fechado.",
            ),
            RowContent::new(
                Level::Ok,
                "Dota 2 encontrado",
                "/steam/steamapps/common/dota 2 beta (versão nativa Linux)",
            ),
            RowContent::new(
                Level::Ok,
                "Arquivo GSI instalado",
                "/steam/gamestate_integration_dota_companion.cfg",
            ),
        ]
    );
}

#[test]
fn missing_install_disables_install_and_shows_error_row() {
    let mut snapshot = logging_snapshot(0);
    snapshot.diagnostics = Some(report(vec![]));

    let content = diagnostics_content(&snapshot, None);

    assert!(!content.can_install);
    assert_eq!(
        content.dota[1],
        RowContent::new(
            Level::Error,
            "Dota 2 não encontrado",
            "Bibliotecas verificadas: /steam. Adicione a sua em extra_libraries no arquivo de \
             configuração.",
        )
    );
}

#[test]
fn before_the_first_diagnostic_shows_checking_and_no_event() {
    let content = diagnostics_content(&AppSnapshot::default(), None);

    assert!(!content.can_install);
    assert_eq!(
        content.dota,
        vec![RowContent::new(Level::Info, "Verificando…", "")]
    );
    assert_eq!(content.json_source, "Nenhum evento recebido ainda.");
}

#[test]
fn install_result_comes_after_the_diagnostic_rows() {
    let mut snapshot = logging_snapshot(0);
    snapshot.last_install = Some(InstallOutcome::NeedsConfirmation(PathBuf::from(
        "/steam/gsi.cfg",
    )));

    let content = diagnostics_content(&snapshot, None);

    assert_eq!(
        content.dota.last(),
        Some(&RowContent::new(
            Level::Warning,
            "Instalação aguardando confirmação",
            "/steam/gsi.cfg já existe e é diferente. Nada foi alterado.",
        ))
    );
    assert_eq!(content.dota.len(), 4);
}

#[test]
fn core_failure_is_the_first_connection_row() {
    let failure = CoreConnectionError::StateClosed.to_diagnostic(&Redactor::default());

    let content = diagnostics_content(&logging_snapshot(0), Some(&failure));

    assert_eq!(content.server[0], RowContent::diagnostic(&failure));
    assert_eq!(
        content.server[1].title,
        "Servidor ouvindo em 127.0.0.1:53000"
    );
}
