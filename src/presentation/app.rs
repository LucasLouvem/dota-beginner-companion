//! Janela principal: barra lateral com as abas e indicador de conexão.

use relm4::adw::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, adw, gtk,
};
use tokio::sync::{mpsc, watch};

use crate::application::event_bus::UiCommand;
use crate::application::state::{AppSnapshot, ServerState};

use super::i18n::{t, tf};
use super::pages::{
    self,
    diagnostics::{DiagnosticsMsg, DiagnosticsPage},
};

/// Canais de comunicação entre a janela e o núcleo.
pub struct CoreHandle {
    pub commands: mpsc::UnboundedSender<UiCommand>,
    pub state: watch::Receiver<AppSnapshot>,
}

/// Componente raiz da janela principal.
pub struct App {
    diagnostics: Controller<DiagnosticsPage>,
}

/// Widgets da janela atualizados a cada novo estado.
pub struct AppWidgets {
    connection: gtk::Label,
}

impl Component for App {
    type Init = CoreHandle;
    type Input = ();
    type Output = ();
    type CommandOutput = Box<AppSnapshot>;
    type Root = adw::ApplicationWindow;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder()
            .title(t("Dota Beginner Companion"))
            .default_width(1000)
            .default_height(720)
            .width_request(360)
            .height_request(400)
            .build()
    }

    fn init(
        core: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let diagnostics = DiagnosticsPage::builder()
            .launch(core.commands.clone())
            .detach();

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        for (name, title, page) in pages::tabs() {
            stack.add_titled(&page, Some(name), &title);
        }
        stack.add_titled(diagnostics.widget(), Some("diagnostics"), &t("Diagnóstico"));
        // A aba Diagnóstico abre por padrão.
        stack.set_visible_child_name("diagnostics");

        let sidebar = gtk::StackSidebar::new();
        sidebar.set_stack(&stack);
        sidebar.set_width_request(180);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&sidebar);
        body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        body.append(&stack);

        let connection = gtk::Label::new(None);
        connection.add_css_class("dim-label");
        let header = adw::HeaderBar::new();
        header.pack_end(&connection);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&body));
        root.set_content(Some(&toolbar));

        let mut state = core.state;
        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    loop {
                        let snapshot = state.borrow_and_update().clone();
                        out.emit(Box::new(snapshot));
                        if state.changed().await.is_err() {
                            break;
                        }
                    }
                })
                .drop_on_shutdown()
        });

        ComponentParts {
            model: App { diagnostics },
            widgets: AppWidgets { connection },
        }
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        snapshot: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        widgets.connection.set_label(&connection_label(&snapshot));
        self.diagnostics.emit(DiagnosticsMsg::Snapshot(snapshot));
    }
}

/// Indicador curto da barra de título. "Conectado" só aparece com eventos
/// reais do Dota recentes; o teste manual não conta.
fn connection_label(snapshot: &AppSnapshot) -> String {
    match (&snapshot.gsi.server, &snapshot.gsi.last_dota_event) {
        (ServerState::Failed(_), _) => t("GSI: servidor com erro"),
        (ServerState::Starting, _) => t("GSI: iniciando…"),
        (ServerState::Listening(_), None) => t("GSI: aguardando o Dota"),
        (ServerState::Listening(_), Some(event)) => {
            let secs = event.at.elapsed().map(|d| d.as_secs()).unwrap_or_default();
            if secs <= 90 {
                t("GSI: recebendo dados do Dota")
            } else {
                tf("GSI: sem dados há {} min", &[&(secs / 60).to_string()])
            }
        }
    }
}
