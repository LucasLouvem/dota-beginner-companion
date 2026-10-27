//! Conteúdo da aba Diagnóstico como tela de saúde, sem widgets: o estado
//! geral, um card para cada coisa que precisa funcionar, o resultado das
//! ações e os detalhes técnicos, que ficam à parte. Os textos são para quem
//! não conhece GSI, JSON ou logs; nada daqui mostra token, payload ou dados
//! pessoais.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::application::diagnostics::{DiagnosticReport, GsiConfigState, InstallCheck};
use crate::application::error::{Diagnostic, Redactor, ToDiagnostic};
use crate::application::event_bus::RejectReason;
use crate::application::state::{
    AppSnapshot, ConnectionTest, GsiLogStatus, InstallOutcome, ReceivedEvent, ServerState,
};
use crate::presentation::i18n::{translate, translate_text, translate_with};
use crate::presentation::widgets::{Level, RowContent, format_time};

/// Sem evento do Dota há mais tempo que isso (o heartbeat é de 30 s), a
/// conexão não é considerada ativa.
pub const RECENT_EVENT_SECS: u64 = 90;

/// Estado geral do aplicativo para acompanhar o Dota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverallState {
    /// O primeiro diagnóstico ainda não terminou.
    Checking,
    Working,
    Attention,
    NotReady,
    CoreUnavailable,
}

impl OverallState {
    pub fn level(self) -> Level {
        match self {
            OverallState::Checking => Level::Info,
            OverallState::Working => Level::Ok,
            OverallState::Attention => Level::Warning,
            OverallState::NotReady | OverallState::CoreUnavailable => Level::Error,
        }
    }

    pub fn title(self) -> String {
        match self {
            OverallState::Checking => translate("Verificando…"),
            OverallState::Working => translate("Tudo funcionando"),
            OverallState::Attention => translate("Atenção necessária"),
            OverallState::NotReady => translate("Não pronto"),
            OverallState::CoreUnavailable => translate("Núcleo indisponível"),
        }
    }

    pub fn description(self) -> String {
        match self {
            OverallState::Checking => translate("Procurando o Dota 2 e conferindo a configuração."),
            OverallState::Working => {
                translate("O aplicativo está pronto para acompanhar o Dota 2.")
            }
            OverallState::Attention => {
                translate("Está quase tudo certo; veja o que pede atenção abaixo.")
            }
            OverallState::NotReady => {
                translate("Falta preparar algo para receber os dados do Dota 2.")
            }
            OverallState::CoreUnavailable => {
                translate("O aplicativo perdeu a comunicação interna. Feche e abra de novo.")
            }
        }
    }
}

/// Como estão chegando os dados do Dota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFlow {
    Receiving,
    /// Nenhum dado chegou desde que o aplicativo abriu.
    Waiting,
    /// Os dados pararam e o Dota está fechado: o esperado depois do jogo.
    Stale,
    /// Os dados pararam com o Dota aberto, ou houve dados recusados depois do
    /// último recebido.
    Unstable,
}

/// Um card da tela: o que se verifica, o estado e uma explicação curta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCard {
    pub level: Level,
    pub title: String,
    pub state: String,
    pub description: String,
}

/// Informações técnicas, mostradas só no bloco secundário e fechado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnicalDetails {
    pub received: String,
    pub recorded: Option<String>,
    pub rejected: Option<String>,
    /// Gravação dos eventos em arquivo: andamento ou falha.
    pub log: Option<RowContent>,
    pub config_path: String,
    /// Origem e horário do último evento (o JSON fica no widget).
    pub json_source: String,
}

/// Tudo o que a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthContent {
    pub overall: OverallState,
    /// Steam, Dota 2, servidor GSI, configuração GSI e recebimento de dados,
    /// nessa ordem.
    pub cards: Vec<HealthCard>,
    /// O botão de instalar só fica ativo com uma instalação do Dota encontrada.
    pub can_install: bool,
    /// Resultado das ações e falhas, em linhas curtas.
    pub results: Vec<RowContent>,
    pub details: TechnicalDetails,
}

/// Conteúdo da tela para o estado, com a falha de comunicação com o núcleo,
/// quando existe.
pub fn health_content(snapshot: &AppSnapshot, core_failure: Option<&Diagnostic>) -> HealthContent {
    let report = snapshot.diagnostics.as_ref();
    let flow = data_flow(
        snapshot,
        report.is_some_and(|report| report.dota_process_running),
    );
    let cards = vec![
        steam_card(report),
        dota_card(report),
        server_card(snapshot),
        config_card(report, &snapshot.redactor),
        data_card(snapshot, flow, report),
    ];
    HealthContent {
        overall: overall_state(snapshot, flow, core_failure.is_some()),
        cards,
        can_install: first_library(snapshot).is_some(),
        results: results(snapshot, core_failure),
        details: details(snapshot),
    }
}

/// Estado geral, na ordem de gravidade.
pub fn overall_state(
    snapshot: &AppSnapshot,
    flow: DataFlow,
    core_unavailable: bool,
) -> OverallState {
    if core_unavailable {
        return OverallState::CoreUnavailable;
    }
    let server_failed = matches!(snapshot.gsi.server, ServerState::Failed(_));
    let Some(report) = &snapshot.diagnostics else {
        return if server_failed {
            OverallState::NotReady
        } else {
            OverallState::Checking
        };
    };
    let config_ready = report
        .installs
        .first()
        .is_some_and(|check| matches!(check.config, GsiConfigState::Matches));
    if server_failed || !config_ready {
        return OverallState::NotReady;
    }
    let action_failed = matches!(
        snapshot.connection_test,
        Some(ConnectionTest::Failed(_) | ConnectionTest::Interrupted(_))
    ) || matches!(
        snapshot.last_install,
        Some(InstallOutcome::Failed(_) | InstallOutcome::Interrupted(_))
    );
    if flow == DataFlow::Unstable || action_failed || snapshot.diagnostics_interrupted.is_some() {
        return OverallState::Attention;
    }
    OverallState::Working
}

/// Situação dos dados recebidos do Dota.
pub fn data_flow(snapshot: &AppSnapshot, dota_running: bool) -> DataFlow {
    let gsi = &snapshot.gsi;
    let rejected_after = |event: Option<SystemTime>| match (gsi.last_rejection, event) {
        (Some((at, _)), Some(event_at)) => at > event_at,
        (Some(_), None) => true,
        (None, _) => false,
    };
    match &gsi.last_dota_event {
        None if rejected_after(None) => DataFlow::Unstable,
        None => DataFlow::Waiting,
        Some(event) => {
            let recent = event
                .at
                .elapsed()
                .is_ok_and(|elapsed| elapsed.as_secs() <= RECENT_EVENT_SECS);
            if rejected_after(Some(event.at)) || (!recent && dota_running) {
                DataFlow::Unstable
            } else if recent {
                DataFlow::Receiving
            } else {
                DataFlow::Stale
            }
        }
    }
}

/// Biblioteca Steam da primeira instalação do Dota encontrada.
pub fn first_library(snapshot: &AppSnapshot) -> Option<PathBuf> {
    snapshot
        .diagnostics
        .as_ref()?
        .installs
        .first()
        .map(|check| check.install.library.clone())
}

/// Causa e resolução num texto só, sem o código nem os detalhes técnicos.
fn plain(diagnostic: &Diagnostic) -> String {
    format!(
        "{} {}",
        translate_text(&diagnostic.cause),
        translate_text(&diagnostic.resolution)
    )
}

fn card(level: Level, title: &str, state: &str, description: &str) -> HealthCard {
    HealthCard {
        level,
        title: translate(title),
        state: translate(state),
        description: translate(description),
    }
}

/// A Steam só é "aberta" quando o processo `steam` foi visto; sem relatório
/// ainda não há o que dizer. Não ser detectada não rebaixa o estado geral.
fn steam_card(report: Option<&DiagnosticReport>) -> HealthCard {
    match report {
        None => card(
            Level::Info,
            "Steam",
            "Estado desconhecido",
            "A verificação ainda não terminou.",
        ),
        Some(report) if report.steam_process_running => {
            card(Level::Ok, "Steam", "Aberta", "A Steam está em execução.")
        }
        Some(_) => card(
            Level::Info,
            "Steam",
            "Não detectada",
            "A Steam não está aberta. Abra-a para iniciar o Dota 2.",
        ),
    }
}

fn dota_card(report: Option<&DiagnosticReport>) -> HealthCard {
    let Some(report) = report else {
        return card(
            Level::Info,
            "Dota 2",
            "Verificando…",
            "Procurando o Dota 2 neste computador.",
        );
    };
    if report.installs.is_empty() {
        return card(
            Level::Error,
            "Dota 2",
            "Instalação não encontrada",
            "O Dota 2 não foi achado nas bibliotecas da Steam. Adicione a sua em extra_libraries no arquivo de configuração do aplicativo.",
        );
    }
    if report.dota_process_running {
        card(
            Level::Ok,
            "Dota 2",
            "Em execução",
            "O jogo foi reconhecido.",
        )
    } else {
        card(
            Level::Info,
            "Dota 2",
            "Não detectado",
            "O jogo não está aberto. Normal quando você não está jogando.",
        )
    }
}

fn server_card(snapshot: &AppSnapshot) -> HealthCard {
    match &snapshot.gsi.server {
        ServerState::Starting => card(
            Level::Info,
            "Servidor GSI",
            "Iniciando",
            "O receptor de dados do Dota está começando.",
        ),
        ServerState::Listening(_) => card(
            Level::Ok,
            "Servidor GSI",
            "Funcionando",
            "O aplicativo está pronto para receber dados do Dota neste computador.",
        ),
        ServerState::Failed(server_error) => HealthCard {
            level: Level::Error,
            title: translate("Servidor GSI"),
            state: translate("Com erro"),
            description: plain(&server_error.to_diagnostic(&snapshot.redactor)),
        },
    }
}

fn config_card(report: Option<&DiagnosticReport>, redactor: &Redactor) -> HealthCard {
    let title = "Configuração GSI";
    let Some(report) = report else {
        return card(
            Level::Info,
            title,
            "Verificando…",
            "Conferindo o arquivo de configuração do Dota.",
        );
    };
    let Some(check) = report.installs.first() else {
        return card(
            Level::Info,
            title,
            "Não verificada",
            "Precisa da instalação do Dota 2 encontrada.",
        );
    };
    config_state_card(check, redactor)
}

fn config_state_card(check: &InstallCheck, redactor: &Redactor) -> HealthCard {
    let title = "Configuração GSI";
    match &check.config {
        GsiConfigState::Matches => card(
            Level::Ok,
            title,
            "Instalada e válida",
            "O Dota está configurado para enviar dados a este aplicativo.",
        ),
        GsiConfigState::Missing => card(
            Level::Warning,
            title,
            "Ausente",
            "Use \"Instalar configuração GSI\" para o Dota poder enviar dados.",
        ),
        GsiConfigState::UriMismatch { .. } => card(
            Level::Warning,
            title,
            "Aponta para outro endereço ou porta",
            "Reinstale a configuração para usar o endereço atual do aplicativo.",
        ),
        GsiConfigState::TokenMismatch => card(
            Level::Warning,
            title,
            "Token divergente",
            "Reinstale a configuração para atualizar a chave de acesso.",
        ),
        GsiConfigState::Failed(file_error) => HealthCard {
            level: Level::Error,
            title: translate(title),
            state: translate("Inválida"),
            description: plain(&file_error.to_diagnostic(redactor)),
        },
    }
}

/// O Dota só envia `player` e `provider` enquanto está no menu ou no lobby:
/// sem o bloco `map` não há fase de partida.
fn in_menu(snapshot: &AppSnapshot) -> bool {
    snapshot
        .match_status
        .game
        .as_ref()
        .is_some_and(|game| game.phase.is_none())
}

fn data_card(
    snapshot: &AppSnapshot,
    flow: DataFlow,
    report: Option<&DiagnosticReport>,
) -> HealthCard {
    let title = "Recebimento de dados";
    match flow {
        DataFlow::Receiving if in_menu(snapshot) => card(
            Level::Ok,
            title,
            "Recebendo dados do menu",
            "O Dota está conectado, mas a partida ainda não começou. Os dados completos chegam a partir da seleção de heróis.",
        ),
        DataFlow::Receiving => card(
            Level::Ok,
            title,
            "Recebendo dados",
            "O aplicativo recebeu dados do Dota há pouco.",
        ),
        DataFlow::Waiting => {
            let running = report.is_some_and(|report| report.dota_process_running);
            if running {
                card(
                    Level::Warning,
                    title,
                    "Aguardando o Dota",
                    "O jogo está aberto, mas nenhum dado chegou. Confira a configuração GSI e a opção de inicialização -gamestateintegration na Steam.",
                )
            } else {
                card(
                    Level::Info,
                    title,
                    "Aguardando o Dota",
                    "Nenhum dado recebido ainda. Abra o Dota 2 e entre numa partida ou lobby.",
                )
            }
        }
        DataFlow::Stale => HealthCard {
            level: Level::Info,
            title: translate(title),
            state: translate("Sem dados recentes"),
            description: translate_with(
                "Os últimos dados chegaram às {}. Normal se o jogo foi fechado.",
                &[&snapshot
                    .gsi
                    .last_dota_event
                    .as_ref()
                    .map(|event| format_time(event.at))
                    .unwrap_or_default()],
            ),
        },
        DataFlow::Unstable => card(
            Level::Warning,
            title,
            "Conexão instável ou interrompida",
            "Os dados pararam de chegar ou foram recusados. Confira a configuração GSI e reinicie o Dota 2.",
        ),
    }
}

/// Resultado das ações e falhas, em linhas curtas; o caminho de um arquivo
/// só aparece quando ajuda a resolver um erro.
fn results(snapshot: &AppSnapshot, core_failure: Option<&Diagnostic>) -> Vec<RowContent> {
    let redactor = &snapshot.redactor;
    let mut rows: Vec<RowContent> = core_failure
        .map(RowContent::diagnostic)
        .into_iter()
        .collect();
    rows.extend(
        snapshot
            .diagnostics_interrupted
            .iter()
            .map(|interrupted| RowContent::diagnostic(&interrupted.to_diagnostic(redactor))),
    );
    if let ServerState::Failed(server_error) = &snapshot.gsi.server {
        rows.push(RowContent::diagnostic(
            &server_error.to_diagnostic(redactor),
        ));
    }
    if let Some(check) = snapshot
        .diagnostics
        .as_ref()
        .and_then(|report| report.installs.first())
        && let GsiConfigState::Failed(file_error) = &check.config
    {
        rows.push(RowContent::diagnostic(&file_error.to_diagnostic(redactor)));
    }
    rows.extend(
        snapshot
            .last_install
            .iter()
            .map(|outcome| install_row(outcome, redactor)),
    );
    match &snapshot.connection_test {
        None => {}
        Some(ConnectionTest::Running) => {
            rows.push(RowContent::new(Level::Info, &translate("Testando a conexão…"), ""))
        }
        Some(ConnectionTest::Passed) => rows.push(RowContent::new(
            Level::Ok,
            &translate("Teste de conexão aprovado"),
            &translate(
                "O servidor local recebeu o evento de teste. Isso não confirma que o Dota está enviando dados.",
            ),
        )),
        Some(ConnectionTest::Failed(test_error)) => {
            rows.push(RowContent::diagnostic(&test_error.to_diagnostic(redactor)))
        }
        Some(ConnectionTest::Interrupted(interrupted)) => {
            rows.push(RowContent::diagnostic(&interrupted.to_diagnostic(redactor)))
        }
    }
    rows
}

fn install_row(outcome: &InstallOutcome, redactor: &Redactor) -> RowContent {
    match outcome {
        InstallOutcome::Created(_) => RowContent::new(
            Level::Ok,
            &translate("Configuração GSI instalada"),
            &translate("Reinicie o Dota 2 para ele passar a enviar dados."),
        ),
        InstallOutcome::Unchanged(_) => RowContent::new(
            Level::Ok,
            &translate("Configuração GSI já estava atualizada"),
            "",
        ),
        InstallOutcome::Replaced { .. } => RowContent::new(
            Level::Ok,
            &translate("Configuração GSI atualizada"),
            &translate("Uma cópia do arquivo anterior foi guardada. Reinicie o Dota 2."),
        ),
        InstallOutcome::NeedsConfirmation(path) => RowContent::new(
            Level::Warning,
            &translate("Instalação aguardando confirmação"),
            &translate_with(
                "{} já existe e é diferente. Nada foi alterado.",
                &[&path.display().to_string()],
            ),
        ),
        InstallOutcome::Failed(install_error) => {
            RowContent::diagnostic(&install_error.to_diagnostic(redactor))
        }
        InstallOutcome::Interrupted(interrupted) => {
            RowContent::diagnostic(&interrupted.to_diagnostic(redactor))
        }
    }
}

/// Evento mais recente e se ele veio do botão de teste.
pub fn latest_event(snapshot: &AppSnapshot) -> Option<(&ReceivedEvent, bool)> {
    let gsi = &snapshot.gsi;
    match (&gsi.last_dota_event, &gsi.last_test_event) {
        (Some(dota_event), Some(test)) if test.at > dota_event.at => Some((test, true)),
        (Some(dota_event), _) => Some((dota_event, false)),
        (None, Some(test)) => Some((test, true)),
        (None, None) => None,
    }
}

fn details(snapshot: &AppSnapshot) -> TechnicalDetails {
    let gsi = &snapshot.gsi;
    TechnicalDetails {
        received: translate_with(
            "Eventos recebidos do Dota: {}",
            &[&gsi.dota_events.to_string()],
        ),
        recorded: snapshot
            .gsi_log
            .as_ref()
            .map(|log| translate_with("Eventos gravados: {}", &[&log.written.to_string()])),
        rejected: gsi.last_rejection.map(|(at, reason)| {
            let reason = match reason {
                RejectReason::InvalidToken => translate("token inválido"),
                RejectReason::InvalidJson => translate("JSON inválido"),
            };
            translate_with(
                "Requisições recusadas: {} (última às {}, {})",
                &[&gsi.rejected.to_string(), &format_time(at), &reason],
            )
        }),
        log: snapshot
            .gsi_log
            .as_ref()
            .map(|log| log_row(log, &snapshot.redactor)),
        config_path: translate_with(
            "Arquivo de configuração do aplicativo: {}",
            &[&snapshot.config_path.display().to_string()],
        ),
        json_source: match latest_event(snapshot) {
            None => translate("Nenhum evento recebido ainda."),
            Some((event, true)) => {
                translate_with("Origem: teste manual · {}", &[&format_time(event.at)])
            }
            Some((event, false)) => {
                translate_with("Origem: Dota 2 · {}", &[&format_time(event.at)])
            }
        },
    }
}

fn log_row(log_status: &GsiLogStatus, redactor: &Redactor) -> RowContent {
    if let Some(stopped) = &log_status.stopped {
        return RowContent::diagnostic(&stopped.to_diagnostic(redactor));
    }
    match &log_status.last_error {
        None => RowContent::new(
            Level::Info,
            &translate("Gravação de eventos ativa"),
            &translate_with(
                "{} — o arquivo contém SteamID e nome do jogador; não compartilhe.",
                &[&log_status.path.display().to_string()],
            ),
        ),
        Some(record_error) => RowContent::diagnostic(&record_error.to_diagnostic(redactor)),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/diagnostics_health_tests.rs"]
mod tests;
