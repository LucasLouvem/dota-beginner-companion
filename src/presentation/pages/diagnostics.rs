//! Tela "Diagnóstico": mostra o que foi detectado e o que falta para o GSI
//! funcionar. Só exibe o estado e envia comandos ao núcleo.

use std::path::PathBuf;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};
use tokio::sync::mpsc::UnboundedSender;

use crate::application::diagnostics::{DiagnosticReport, GsiConfigState, InstallCheck};
use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, log_error};
use crate::application::event_bus::{CoreConnectionError, RejectReason, UiCommand};
use crate::application::state::{
    AppSnapshot, ConnectionTest, GsiLogStatus, InstallOutcome, ReceivedEvent, ServerState,
};
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{
    Level, RowContent, RowSection, boxed_list, format_time, section_title, status_row,
};

use super::send_command;

/// Sem evento do Dota há mais tempo que isso (heartbeat é 30 s), não
/// consideramos a conexão ativa.
const RECENT_EVENT_SECS: u64 = 90;

/// Conteúdo da página que muda com o estado, na ordem exibida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsContent {
    /// O botão de instalar só fica ativo com uma instalação do Dota encontrada.
    pub can_install: bool,
    pub config_subtitle: String,
    pub dota: Vec<RowContent>,
    pub server: Vec<RowContent>,
    /// Origem e horário do último JSON recebido.
    pub json_source: String,
}

/// Página Diagnóstico.
pub struct DiagnosticsPage {
    commands: UnboundedSender<UiCommand>,
    snapshot: AppSnapshot,
    handled_install: u64,
    shown_raw: Option<Arc<str>>,
    /// Falha de comunicação com o núcleo. Fica na própria página porque o
    /// canal que falhou não pode trazer o aviso.
    core_failure: Option<CoreConnectionError>,
}

/// Mensagens recebidas pela página Diagnóstico.
#[derive(Debug)]
pub enum DiagnosticsMessage {
    Snapshot(Box<AppSnapshot>),
    /// O núcleo parou de publicar o estado.
    CoreStopped,
    Run,
    Test,
    Install,
    ConfirmOverwrite,
}

/// Widgets da página Diagnóstico, com o que cada lista mostra agora.
pub struct DiagnosticsWidgets {
    install_button: gtk::Button,
    dota: RowSection,
    server: RowSection,
    config_row: adw::ActionRow,
    json_source: gtk::Label,
    json_buffer: gtk::TextBuffer,
}

impl Component for DiagnosticsPage {
    type Init = UnboundedSender<UiCommand>;
    type Input = DiagnosticsMessage;
    type Output = ();
    type CommandOutput = ();
    type Root = gtk::ScrolledWindow;
    type Widgets = DiagnosticsWidgets;

    fn init_root() -> Self::Root {
        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build()
    }

    fn init(
        commands: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(16);
        content.set_margin_end(16);

        let title = gtk::Label::new(Some(&translate("Diagnóstico")));
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let intro = gtk::Label::new(Some(&translate(
            "Verifica se o Dota 2 foi encontrado, se o arquivo do Game State Integration (GSI) \
             está instalado e se o aplicativo está recebendo dados. Nada aqui altera o jogo.",
        )));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");
        content.append(&title);
        content.append(&intro);

        let buttons = gtk::FlowBox::new();
        buttons.set_selection_mode(gtk::SelectionMode::None);
        buttons.set_column_spacing(8);
        buttons.set_row_spacing(8);
        buttons.set_max_children_per_line(3);
        let run_button = gtk::Button::with_label(&translate("Executar diagnóstico"));
        let test_button = gtk::Button::with_label(&translate("Testar conexão"));
        let install_button = gtk::Button::with_label(&translate("Instalar configuração GSI"));
        install_button.add_css_class("suggested-action");
        install_button.set_sensitive(false);
        {
            let button_sender = sender.clone();
            run_button.connect_clicked(move |_| button_sender.input(DiagnosticsMessage::Run));
            let button_sender = sender.clone();
            test_button.connect_clicked(move |_| button_sender.input(DiagnosticsMessage::Test));
            let button_sender = sender.clone();
            install_button
                .connect_clicked(move |_| button_sender.input(DiagnosticsMessage::Install));
        }
        for button in [&run_button, &test_button, &install_button] {
            buttons.insert(button, -1);
        }
        content.append(&buttons);

        let dota_list = boxed_list();
        content.append(&section_title(&translate("Dota 2")));
        content.append(&dota_list);

        let server_list = boxed_list();
        content.append(&section_title(&translate("Conexão GSI")));
        content.append(&server_list);

        let requirements = boxed_list();
        requirements.append(&status_row(
            Level::Info,
            &translate("Opção de inicialização obrigatória"),
            &translate(
                "Na Steam: Dota 2 → Propriedades → Geral → Opções de inicialização → \
                adicione -gamestateintegration. Sem ela o Dota não envia dados.",
            ),
        ));
        requirements.append(&status_row(
            Level::Info,
            &translate("O que o GSI informa"),
            &translate(
                "Os dados recebidos incluem fase da partida, herói, itens, habilidades e \
                estatísticas do próprio jogador. \
                \"Partida encontrada\" não é informada diretamente pelo GSI. \
                Nas partidas observadas, o menu também gerou eventos, sem o bloco map.",
            ),
        ));
        let config_row = status_row(Level::Info, &translate("Configuração do aplicativo"), "");
        requirements.append(&config_row);
        content.append(&section_title(&translate("Requisitos e limitações")));
        content.append(&requirements);

        let json_source = gtk::Label::new(None);
        json_source.set_xalign(0.0);
        json_source.add_css_class("dim-label");
        let json_view = gtk::TextView::builder()
            .editable(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(8)
            .right_margin(8)
            .build();
        let json_buffer = json_view.buffer();
        let json_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(260)
            .child(&json_view)
            .build();
        json_scroll.add_css_class("card");
        let json_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        json_box.append(&json_source);
        json_box.append(&json_scroll);
        let expander = gtk::Expander::new(Some(&translate("Último JSON recebido")));
        expander.set_child(Some(&json_box));
        content.append(&expander);

        let clamp = adw::Clamp::builder()
            .maximum_size(860)
            .child(&content)
            .build();
        root.set_child(Some(&clamp));

        let model = DiagnosticsPage {
            commands,
            snapshot: AppSnapshot::default(),
            handled_install: 0,
            shown_raw: None,
            core_failure: None,
        };
        let widgets = DiagnosticsWidgets {
            install_button,
            dota: RowSection::new(dota_list),
            server: RowSection::new(server_list),
            config_row,
            json_source,
            json_buffer,
        };
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            DiagnosticsMessage::Snapshot(snapshot) => {
                self.snapshot = *snapshot;
                if self.snapshot.install_attempt != self.handled_install {
                    self.handled_install = self.snapshot.install_attempt;
                    if let Some(InstallOutcome::NeedsConfirmation(path)) =
                        &self.snapshot.last_install
                    {
                        confirm_overwrite(root, path.clone(), sender);
                    }
                }
                self.render(widgets);
            }
            DiagnosticsMessage::CoreStopped => {
                self.report_core_failure(widgets, CoreConnectionError::StateClosed);
            }
            DiagnosticsMessage::Run => self.send(widgets, UiCommand::RunDiagnostics),
            DiagnosticsMessage::Test => self.send(widgets, UiCommand::TestConnection),
            DiagnosticsMessage::Install => {
                if let Some(root) = self.first_library() {
                    self.send(
                        widgets,
                        UiCommand::InstallGsiConfig {
                            install_root: root,
                            overwrite: false,
                        },
                    );
                }
            }
            DiagnosticsMessage::ConfirmOverwrite => {
                if let Some(root) = self.first_library() {
                    self.send(
                        widgets,
                        UiCommand::InstallGsiConfig {
                            install_root: root,
                            overwrite: true,
                        },
                    );
                }
            }
        }
    }
}

impl DiagnosticsPage {
    fn send(&mut self, widgets: &mut DiagnosticsWidgets, command: UiCommand) {
        if let Err(core_error) = send_command(&self.commands, command) {
            self.report_core_failure(widgets, core_error);
        }
    }

    /// Mostra a falha de comunicação com o núcleo, mantendo a primeira.
    fn report_core_failure(
        &mut self,
        widgets: &mut DiagnosticsWidgets,
        core_error: CoreConnectionError,
    ) {
        if self.core_failure.is_none() {
            log_error(
                &core_error.to_diagnostic(&self.snapshot.redactor),
                "user interface lost the connection to the core",
            );
            self.core_failure = Some(core_error);
        }
        self.render(widgets);
    }

    fn first_library(&self) -> Option<PathBuf> {
        first_library(&self.snapshot)
    }

    /// Atualiza só o que mudou desde o último estado exibido.
    fn render(&mut self, widgets: &mut DiagnosticsWidgets) {
        let snapshot = &self.snapshot;
        let core_failure = self
            .core_failure
            .as_ref()
            .map(|core_error| core_error.to_diagnostic(&snapshot.redactor));
        let content = diagnostics_content(snapshot, core_failure.as_ref());

        if widgets.install_button.is_sensitive() != content.can_install {
            widgets.install_button.set_sensitive(content.can_install);
        }
        if widgets.config_row.subtitle().as_deref() != Some(content.config_subtitle.as_str()) {
            widgets.config_row.set_subtitle(&content.config_subtitle);
        }
        widgets.dota.show(content.dota);
        widgets.server.show(content.server);

        let raw = latest_event(snapshot).map(|(event, _)| Arc::clone(&event.raw));
        if raw.as_ref().map(Arc::as_ptr) != self.shown_raw.as_ref().map(Arc::as_ptr) {
            widgets.json_buffer.set_text(raw.as_deref().unwrap_or(""));
            self.shown_raw = raw;
        }
        if widgets.json_source.label().as_str() != content.json_source {
            widgets.json_source.set_label(&content.json_source);
        }
    }
}

/// Biblioteca Steam da primeira instalação do Dota encontrada.
fn first_library(snapshot: &AppSnapshot) -> Option<PathBuf> {
    snapshot
        .diagnostics
        .as_ref()?
        .installs
        .first()
        .map(|check| check.install.library.clone())
}

/// Conteúdo da página para o estado, com a falha de comunicação com o
/// núcleo, quando existe, como primeira linha da conexão GSI.
pub fn diagnostics_content(
    snapshot: &AppSnapshot,
    core_failure: Option<&Diagnostic>,
) -> DiagnosticsContent {
    let mut dota: Vec<RowContent> = snapshot
        .diagnostics_interrupted
        .iter()
        .map(|interrupted| RowContent::diagnostic(&interrupted.to_diagnostic(&snapshot.redactor)))
        .collect();
    match &snapshot.diagnostics {
        None => dota.push(RowContent::new(Level::Info, &translate("Verificando…"), "")),
        Some(report) => dota.extend(dota_rows(report, &snapshot.redactor)),
    }
    dota.extend(
        snapshot
            .last_install
            .iter()
            .map(|outcome| install_row(outcome, &snapshot.redactor)),
    );

    let mut server: Vec<RowContent> = core_failure
        .map(RowContent::diagnostic)
        .into_iter()
        .collect();
    server.extend(server_rows(snapshot));

    let json_source = match latest_event(snapshot) {
        None => translate("Nenhum evento recebido ainda."),
        Some((event, true)) => {
            translate_with("Origem: teste manual · {}", &[&format_time(event.at)])
        }
        Some((event, false)) => translate_with("Origem: Dota 2 · {}", &[&format_time(event.at)]),
    };

    DiagnosticsContent {
        can_install: first_library(snapshot).is_some(),
        config_subtitle: translate_with(
            "{} — guarda a porta e o token do GSI. Não compartilhe este arquivo.",
            &[&snapshot.config_path.display().to_string()],
        ),
        dota,
        server,
        json_source,
    }
}

/// Evento mais recente e se ele veio do botão de teste.
fn latest_event(snapshot: &AppSnapshot) -> Option<(&ReceivedEvent, bool)> {
    let gsi = &snapshot.gsi;
    match (&gsi.last_dota_event, &gsi.last_test_event) {
        (Some(dota_event), Some(test)) if test.at > dota_event.at => Some((test, true)),
        (Some(dota_event), _) => Some((dota_event, false)),
        (None, Some(test)) => Some((test, true)),
        (None, None) => None,
    }
}

fn dota_rows(report: &DiagnosticReport, redactor: &Redactor) -> Vec<RowContent> {
    let mut rows = Vec::new();
    rows.push(if report.dota_process_running {
        RowContent::new(
            Level::Ok,
            &translate("Dota 2 está aberto"),
            &translate("Detectado pela lista de processos do sistema (dota2)."),
        )
    } else {
        RowContent::new(
            Level::Info,
            &translate("Dota 2 não está aberto"),
            &translate("Nenhum processo dota2 encontrado. Normal se o jogo estiver fechado."),
        )
    });

    if report.installs.is_empty() {
        let searched = if report.searched_libraries.is_empty() {
            translate("nenhuma biblioteca Steam encontrada")
        } else {
            report
                .searched_libraries
                .iter()
                .map(|library| library.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        rows.push(RowContent::new(
            Level::Error,
            &translate("Dota 2 não encontrado"),
            &translate_with(
                "Bibliotecas verificadas: {}. Adicione a sua em extra_libraries no arquivo de configuração.",
                &[&searched],
            ),
        ));
    }

    for check in &report.installs {
        let install = &check.install;
        let kind = match (install.native_binary, install.proton_prefix) {
            (true, false) => translate("versão nativa Linux"),
            (true, true) => translate("versão nativa Linux; também há prefixo Proton"),
            (false, true) => translate("prefixo Proton"),
            (false, false) => translate("tipo de execução desconhecido"),
        };
        rows.push(RowContent::new(
            Level::Ok,
            &translate("Dota 2 encontrado"),
            &format!("{} ({kind})", install.game_root.display()),
        ));

        rows.push(config_row(check, redactor));
    }
    rows
}

fn config_row(check: &InstallCheck, redactor: &Redactor) -> RowContent {
    let path = check.config_path.display().to_string();
    match &check.config {
        GsiConfigState::Missing => RowContent::new(
            Level::Warning,
            &translate("Arquivo GSI não instalado"),
            &translate_with("{} — use \"Instalar configuração GSI\".", &[&path]),
        ),
        GsiConfigState::Matches => {
            RowContent::new(Level::Ok, &translate("Arquivo GSI instalado"), &path)
        }
        GsiConfigState::UriMismatch { found } => RowContent::new(
            Level::Warning,
            &translate("Arquivo GSI aponta para outro endereço"),
            &translate_with(
                "{} — encontrado: {}. Reinstale para usar a porta atual.",
                &[&path, found.as_deref().unwrap_or("?")],
            ),
        ),
        GsiConfigState::TokenMismatch => RowContent::new(
            Level::Warning,
            &translate("Token do arquivo GSI diferente do aplicativo"),
            &translate_with("{} — reinstale para atualizar o token.", &[&path]),
        ),
        GsiConfigState::Failed(file_error) => {
            RowContent::diagnostic(&file_error.to_diagnostic(redactor))
        }
    }
}

fn install_row(outcome: &InstallOutcome, redactor: &Redactor) -> RowContent {
    match outcome {
        InstallOutcome::Created(path) => RowContent::new(
            Level::Ok,
            &translate("Arquivo GSI criado"),
            &translate_with(
                "{} — reinicie o Dota 2 para ele ser lido.",
                &[&path.display().to_string()],
            ),
        ),
        InstallOutcome::Unchanged(path) => RowContent::new(
            Level::Ok,
            &translate("Arquivo GSI já estava atualizado"),
            &path.display().to_string(),
        ),
        InstallOutcome::Replaced { path, backup } => RowContent::new(
            Level::Ok,
            &translate("Arquivo GSI substituído"),
            &translate_with(
                "{} — cópia do arquivo anterior em {}. Reinicie o Dota 2.",
                &[&path.display().to_string(), &backup.display().to_string()],
            ),
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

fn server_rows(snapshot: &AppSnapshot) -> Vec<RowContent> {
    let gsi = &snapshot.gsi;
    let mut rows = vec![match &gsi.server {
        ServerState::Starting => {
            RowContent::new(Level::Info, &translate("Iniciando servidor local…"), "")
        }
        ServerState::Listening(address) => RowContent::new(
            Level::Ok,
            &translate_with("Servidor ouvindo em {}", &[&address.to_string()]),
            &translate("Aceita apenas conexões deste computador e com o token correto."),
        ),
        ServerState::Failed(server_error) => {
            RowContent::diagnostic(&server_error.to_diagnostic(&snapshot.redactor))
        }
    }];

    rows.push(match &gsi.last_dota_event {
        None => RowContent::new(
            Level::Warning,
            &translate("Nenhum evento recebido do Dota ainda"),
            &translate(
                "Confira a opção -gamestateintegration e o arquivo GSI, reinicie o Dota e entre em \
                uma partida ou lobby (inclusive contra bots).",
            ),
        ),
        Some(event) => {
            let recent = event
                .at
                .elapsed()
                .is_ok_and(|elapsed| elapsed.as_secs() <= RECENT_EVENT_SECS);
            RowContent::new(
                if recent { Level::Ok } else { Level::Info },
                &translate_with(
                    "Eventos recebidos do Dota: {}",
                    &[&gsi.dota_events.to_string()],
                ),
                &if recent {
                    translate_with("Último: {}", &[&format_time(event.at)])
                } else {
                    translate_with(
                        "Último: {} — sem eventos recentes.",
                        &[&format_time(event.at)],
                    )
                },
            )
        }
    });

    if let Some(log_status) = &snapshot.gsi_log {
        rows.push(gsi_log_row(log_status, &snapshot.redactor));
    }

    if let Some((at, reason)) = gsi.last_rejection {
        let reason = match reason {
            RejectReason::InvalidToken => translate("token inválido"),
            RejectReason::InvalidJson => translate("JSON inválido"),
        };
        rows.push(RowContent::new(
            Level::Error,
            &translate_with("Requisições recusadas: {}", &[&gsi.rejected.to_string()]),
            &translate_with("Última: {} ({})", &[&format_time(at), &reason]),
        ));
    }

    match &snapshot.connection_test {
        None => {}
        Some(ConnectionTest::Running) => {
            rows.push(RowContent::new(Level::Info, &translate("Testando conexão…"), ""))
        }
        Some(ConnectionTest::Passed) => rows.push(RowContent::new(
            Level::Ok,
            &translate("Teste de conexão bem-sucedido"),
            &translate("O servidor local recebeu o evento de teste. Isso não confirma que o Dota está enviando dados."),
        )),
        Some(ConnectionTest::Failed(test_error)) => {
            rows.push(RowContent::diagnostic(&test_error.to_diagnostic(&snapshot.redactor)))
        }
        Some(ConnectionTest::Interrupted(interrupted)) => {
            rows.push(RowContent::diagnostic(&interrupted.to_diagnostic(&snapshot.redactor)))
        }
    }
    rows
}

fn gsi_log_row(log_status: &GsiLogStatus, redactor: &Redactor) -> RowContent {
    if let Some(stopped) = &log_status.stopped {
        return RowContent::diagnostic(&stopped.to_diagnostic(redactor));
    }
    let log_path = log_status.path.display().to_string();
    match &log_status.last_error {
        None => RowContent::new(
            Level::Info,
            &translate_with(
                "Gravando eventos do Dota: {}",
                &[&log_status.written.to_string()],
            ),
            &translate_with(
                "{} — o arquivo contém SteamID e nome do jogador; não compartilhe.",
                &[&log_path],
            ),
        ),
        Some(record_error) => RowContent::diagnostic(&record_error.to_diagnostic(redactor)),
    }
}

fn confirm_overwrite(
    root: &gtk::ScrolledWindow,
    path: PathBuf,
    sender: ComponentSender<DiagnosticsPage>,
) {
    let dialog = adw::AlertDialog::new(
        Some(&translate("Substituir arquivo GSI existente?")),
        Some(&translate_with(
            "{} já existe com conteúdo diferente. Uma cópia de segurança será criada antes de substituir.",
            &[&path.display().to_string()],
        )),
    );
    dialog.add_response("cancel", &translate("Cancelar"));
    dialog.add_response("replace", &translate("Substituir com backup"));
    dialog.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(Some("replace"), move |_, _| {
        sender.input(DiagnosticsMessage::ConfirmOverwrite);
    });
    dialog.present(Some(root));
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/diagnostics_tests.rs"]
mod tests;
