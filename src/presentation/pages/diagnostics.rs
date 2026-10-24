//! Tela "Diagnóstico": mostra o que foi detectado e o que falta para o GSI
//! funcionar. Só exibe o estado e envia comandos ao núcleo.

use std::path::PathBuf;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};
use tokio::sync::mpsc::UnboundedSender;

use crate::application::event_bus::{RejectReason, UiCommand};
use crate::application::state::{
    AppSnapshot, ConnectionTest, InstallOutcome, ReceivedEvent, ServerState,
};
use crate::domain::diagnostics::{DiagnosticReport, GsiConfigState};
use crate::domain::gsi_config::ConfigMatch;
use crate::presentation::i18n::{t, tf};
use crate::presentation::widgets::{Level, boxed_list, format_time, section_title, status_row};

/// Sem evento do Dota há mais tempo que isso (heartbeat é 30 s), não
/// consideramos a conexão ativa.
const RECENT_EVENT_SECS: u64 = 90;

/// Página Diagnóstico.
pub struct DiagnosticsPage {
    commands: UnboundedSender<UiCommand>,
    snapshot: AppSnapshot,
    handled_install: u64,
    shown_raw: Option<Arc<str>>,
}

/// Mensagens recebidas pela página Diagnóstico.
#[derive(Debug)]
pub enum DiagnosticsMsg {
    Snapshot(Box<AppSnapshot>),
    Run,
    Test,
    Install,
    ConfirmOverwrite,
}

/// Widgets da página Diagnóstico reconstruídos a cada novo estado.
pub struct DiagnosticsWidgets {
    install_button: gtk::Button,
    dota_list: gtk::ListBox,
    server_list: gtk::ListBox,
    config_row: adw::ActionRow,
    json_source: gtk::Label,
    json_buffer: gtk::TextBuffer,
}

impl Component for DiagnosticsPage {
    type Init = UnboundedSender<UiCommand>;
    type Input = DiagnosticsMsg;
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

        let title = gtk::Label::new(Some(&t("Diagnóstico")));
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let intro = gtk::Label::new(Some(&t(
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
        let run_button = gtk::Button::with_label(&t("Executar diagnóstico"));
        let test_button = gtk::Button::with_label(&t("Testar conexão"));
        let install_button = gtk::Button::with_label(&t("Instalar configuração GSI"));
        install_button.add_css_class("suggested-action");
        install_button.set_sensitive(false);
        {
            let s = sender.clone();
            run_button.connect_clicked(move |_| s.input(DiagnosticsMsg::Run));
            let s = sender.clone();
            test_button.connect_clicked(move |_| s.input(DiagnosticsMsg::Test));
            let s = sender.clone();
            install_button.connect_clicked(move |_| s.input(DiagnosticsMsg::Install));
        }
        for b in [&run_button, &test_button, &install_button] {
            buttons.insert(b, -1);
        }
        content.append(&buttons);

        let dota_list = boxed_list();
        content.append(&section_title(&t("Dota 2")));
        content.append(&dota_list);

        let server_list = boxed_list();
        content.append(&section_title(&t("Conexão GSI")));
        content.append(&server_list);

        let requirements = boxed_list();
        requirements.append(&status_row(
            Level::Info,
            &t("Opção de inicialização obrigatória"),
            &t(
                "Na Steam: Dota 2 → Propriedades → Geral → Opções de inicialização → \
                adicione -gamestateintegration. Sem ela o Dota não envia dados.",
            ),
        ));
        requirements.append(&status_row(
            Level::Info,
            &t("O que o GSI informa"),
            &t(
                "Confirmado pelo jogo: fase da partida (seleção de herói, pré-jogo, em andamento, \
                pós-jogo), seu herói, seus itens, suas habilidades e suas estatísticas. \
                Não informado: \"partida encontrada\" (será apenas inferido, recurso experimental) \
                e dados de outros jogadores. O Dota normalmente não envia nada no menu principal.",
            ),
        ));
        let config_row = status_row(Level::Info, &t("Configuração do aplicativo"), "");
        requirements.append(&config_row);
        content.append(&section_title(&t("Requisitos e limitações")));
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
        let expander = gtk::Expander::new(Some(&t("Último JSON recebido")));
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
        };
        let widgets = DiagnosticsWidgets {
            install_button,
            dota_list,
            server_list,
            config_row,
            json_source,
            json_buffer,
        };
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            DiagnosticsMsg::Snapshot(snapshot) => {
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
            DiagnosticsMsg::Run => self.send(UiCommand::RunDiagnostics),
            DiagnosticsMsg::Test => self.send(UiCommand::TestConnection),
            DiagnosticsMsg::Install => {
                if let Some(root) = self.first_library() {
                    self.send(UiCommand::InstallGsiConfig {
                        install_root: root,
                        overwrite: false,
                    });
                }
            }
            DiagnosticsMsg::ConfirmOverwrite => {
                if let Some(root) = self.first_library() {
                    self.send(UiCommand::InstallGsiConfig {
                        install_root: root,
                        overwrite: true,
                    });
                }
            }
        }
    }
}

impl DiagnosticsPage {
    fn send(&self, command: UiCommand) {
        let _ = self.commands.send(command);
    }

    fn first_library(&self) -> Option<PathBuf> {
        self.snapshot
            .diagnostics
            .as_ref()?
            .installs
            .first()
            .map(|c| c.install.library.clone())
    }

    fn render(&mut self, widgets: &DiagnosticsWidgets) {
        let snapshot = &self.snapshot;
        widgets
            .install_button
            .set_sensitive(self.first_library().is_some());
        widgets.config_row.set_subtitle(&tf(
            "{} — guarda a porta e o token do GSI. Não compartilhe este arquivo.",
            &[&snapshot.config_path.display().to_string()],
        ));

        widgets.dota_list.remove_all();
        match &snapshot.diagnostics {
            None => widgets
                .dota_list
                .append(&status_row(Level::Info, &t("Verificando…"), "")),
            Some(report) => {
                for row in dota_rows(report) {
                    widgets.dota_list.append(&row);
                }
            }
        }
        if let Some(outcome) = &snapshot.last_install {
            widgets.dota_list.append(&install_row(outcome));
        }

        widgets.server_list.remove_all();
        for row in server_rows(snapshot) {
            widgets.server_list.append(&row);
        }

        let latest = latest_event(snapshot);
        let raw = latest.map(|(e, _)| Arc::clone(&e.raw));
        if raw.as_ref().map(Arc::as_ptr) != self.shown_raw.as_ref().map(Arc::as_ptr) {
            widgets.json_buffer.set_text(raw.as_deref().unwrap_or(""));
            self.shown_raw = raw;
        }
        widgets.json_source.set_label(&match latest {
            None => t("Nenhum evento recebido ainda."),
            Some((e, true)) => tf("Origem: teste manual · {}", &[&format_time(e.at)]),
            Some((e, false)) => tf("Origem: Dota 2 · {}", &[&format_time(e.at)]),
        });
    }
}

/// Evento mais recente e se ele veio do botão de teste.
fn latest_event(snapshot: &AppSnapshot) -> Option<(&ReceivedEvent, bool)> {
    let gsi = &snapshot.gsi;
    match (&gsi.last_dota_event, &gsi.last_test_event) {
        (Some(d), Some(test)) if test.at > d.at => Some((test, true)),
        (Some(d), _) => Some((d, false)),
        (None, Some(test)) => Some((test, true)),
        (None, None) => None,
    }
}

fn dota_rows(report: &DiagnosticReport) -> Vec<adw::ActionRow> {
    let mut rows = Vec::new();
    rows.push(if report.dota_process_running {
        status_row(
            Level::Ok,
            &t("Dota 2 está aberto"),
            &t("Detectado pela lista de processos do sistema (dota2)."),
        )
    } else {
        status_row(
            Level::Info,
            &t("Dota 2 não está aberto"),
            &t("Nenhum processo dota2 encontrado. Normal se o jogo estiver fechado."),
        )
    });

    if report.installs.is_empty() {
        let searched = if report.searched_libraries.is_empty() {
            t("nenhuma biblioteca Steam encontrada")
        } else {
            report
                .searched_libraries
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        rows.push(status_row(
            Level::Error,
            &t("Dota 2 não encontrado"),
            &tf(
                "Bibliotecas verificadas: {}. Adicione a sua em extra_libraries no arquivo de configuração.",
                &[&searched],
            ),
        ));
    }

    for check in &report.installs {
        let install = &check.install;
        let kind = match (install.native_binary, install.proton_prefix) {
            (true, false) => t("versão nativa Linux"),
            (true, true) => t("versão nativa Linux; também há prefixo Proton"),
            (false, true) => t("prefixo Proton"),
            (false, false) => t("tipo de execução desconhecido"),
        };
        rows.push(status_row(
            Level::Ok,
            &t("Dota 2 encontrado"),
            &format!("{} ({kind})", install.game_root.display()),
        ));

        let path = check.config_path.display().to_string();
        let (level, title, detail) = match &check.config {
            GsiConfigState::Missing => (
                Level::Warning,
                t("Arquivo GSI não instalado"),
                tf("{} — use \"Instalar configuração GSI\".", &[&path]),
            ),
            GsiConfigState::Present(ConfigMatch::Matches) => {
                (Level::Ok, t("Arquivo GSI instalado"), path)
            }
            GsiConfigState::Present(ConfigMatch::UriMismatch { found }) => (
                Level::Warning,
                t("Arquivo GSI aponta para outro endereço"),
                tf(
                    "{} — encontrado: {}. Reinstale para usar a porta atual.",
                    &[&path, found.as_deref().unwrap_or("?")],
                ),
            ),
            GsiConfigState::Present(ConfigMatch::TokenMismatch) => (
                Level::Warning,
                t("Token do arquivo GSI diferente do aplicativo"),
                tf("{} — reinstale para atualizar o token.", &[&path]),
            ),
            GsiConfigState::Present(ConfigMatch::Unparsable(e)) => (
                Level::Error,
                t("Arquivo GSI inválido"),
                format!("{path} — {e}"),
            ),
            GsiConfigState::Unreadable(e) => (
                Level::Error,
                t("Não foi possível ler o arquivo GSI"),
                format!("{path} — {e}"),
            ),
        };
        rows.push(status_row(level, &title, &detail));
    }
    rows
}

fn install_row(outcome: &InstallOutcome) -> adw::ActionRow {
    match outcome {
        InstallOutcome::Created(path) => status_row(
            Level::Ok,
            &t("Arquivo GSI criado"),
            &tf(
                "{} — reinicie o Dota 2 para ele ser lido.",
                &[&path.display().to_string()],
            ),
        ),
        InstallOutcome::Unchanged(path) => status_row(
            Level::Ok,
            &t("Arquivo GSI já estava atualizado"),
            &path.display().to_string(),
        ),
        InstallOutcome::Replaced { path, backup } => status_row(
            Level::Ok,
            &t("Arquivo GSI substituído"),
            &tf(
                "{} — cópia do arquivo anterior em {}. Reinicie o Dota 2.",
                &[&path.display().to_string(), &backup.display().to_string()],
            ),
        ),
        InstallOutcome::NeedsConfirmation(path) => status_row(
            Level::Warning,
            &t("Instalação aguardando confirmação"),
            &tf(
                "{} já existe e é diferente. Nada foi alterado.",
                &[&path.display().to_string()],
            ),
        ),
        InstallOutcome::Failed { path, error } => status_row(
            Level::Error,
            &t("Falha ao instalar o arquivo GSI"),
            &format!("{} — {error}", path.display()),
        ),
    }
}

fn server_rows(snapshot: &AppSnapshot) -> Vec<adw::ActionRow> {
    let gsi = &snapshot.gsi;
    let mut rows = vec![match &gsi.server {
        ServerState::Starting => status_row(Level::Info, &t("Iniciando servidor local…"), ""),
        ServerState::Listening(addr) => status_row(
            Level::Ok,
            &tf("Servidor ouvindo em {}", &[&addr.to_string()]),
            &t("Aceita apenas conexões deste computador e com o token correto."),
        ),
        ServerState::Failed(e) => status_row(
            Level::Error,
            &t("Servidor local não iniciou"),
            &tf(
                "{} — a porta pode estar em uso. Altere gsi_port na configuração e reinstale o arquivo GSI.",
                &[e],
            ),
        ),
    }];

    rows.push(match &gsi.last_dota_event {
        None => status_row(
            Level::Warning,
            &t("Nenhum evento recebido do Dota ainda"),
            &t(
                "Confira a opção -gamestateintegration e o arquivo GSI, reinicie o Dota e entre em \
                uma partida ou lobby (inclusive contra bots).",
            ),
        ),
        Some(event) => {
            let recent = event
                .at
                .elapsed()
                .is_ok_and(|d| d.as_secs() <= RECENT_EVENT_SECS);
            status_row(
                if recent { Level::Ok } else { Level::Info },
                &tf(
                    "Eventos recebidos do Dota: {}",
                    &[&gsi.dota_events.to_string()],
                ),
                &if recent {
                    tf("Último: {}", &[&format_time(event.at)])
                } else {
                    tf(
                        "Último: {} — sem eventos recentes.",
                        &[&format_time(event.at)],
                    )
                },
            )
        }
    });

    if let Some((at, reason)) = gsi.last_rejection {
        let reason = match reason {
            RejectReason::InvalidToken => t("token inválido"),
            RejectReason::InvalidJson => t("JSON inválido"),
        };
        rows.push(status_row(
            Level::Error,
            &tf("Requisições recusadas: {}", &[&gsi.rejected.to_string()]),
            &tf("Última: {} ({})", &[&format_time(at), &reason]),
        ));
    }

    match &snapshot.connection_test {
        None => {}
        Some(ConnectionTest::Running) => {
            rows.push(status_row(Level::Info, &t("Testando conexão…"), ""))
        }
        Some(ConnectionTest::Passed) => rows.push(status_row(
            Level::Ok,
            &t("Teste de conexão bem-sucedido"),
            &t("O servidor local recebeu o evento de teste. Isso não confirma que o Dota está enviando dados."),
        )),
        Some(ConnectionTest::Failed(e)) => rows.push(status_row(
            Level::Error,
            &t("Teste de conexão falhou"),
            e,
        )),
    }
    rows
}

fn confirm_overwrite(
    root: &gtk::ScrolledWindow,
    path: PathBuf,
    sender: ComponentSender<DiagnosticsPage>,
) {
    let dialog = adw::AlertDialog::new(
        Some(&t("Substituir arquivo GSI existente?")),
        Some(&tf(
            "{} já existe com conteúdo diferente. Uma cópia de segurança será criada antes de substituir.",
            &[&path.display().to_string()],
        )),
    );
    dialog.add_response("cancel", &t("Cancelar"));
    dialog.add_response("replace", &t("Substituir com backup"));
    dialog.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(Some("replace"), move |_, _| {
        sender.input(DiagnosticsMsg::ConfirmOverwrite);
    });
    dialog.present(Some(root));
}
