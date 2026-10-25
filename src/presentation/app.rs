//! Janela principal: barra lateral com as abas e indicador de conexão.

use relm4::adw::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, adw, gtk,
};
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use crate::application::event_bus::UiCommand;
use crate::application::state::{AppSnapshot, ServerState};
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::item_catalog::ItemCatalog;

use super::i18n::{translate, translate_with};
use super::pages::{
    self,
    diagnostics::{DiagnosticsMessage, DiagnosticsPage},
    hero::{HeroMessage, HeroPage, HeroPageInit},
};

/// Canais de comunicação entre a janela e o núcleo, e os dados embutidos.
pub struct CoreHandle {
    pub commands: mpsc::UnboundedSender<UiCommand>,
    pub state: watch::Receiver<AppSnapshot>,
    pub heroes: Arc<HeroCatalog>,
    pub items: Arc<ItemCatalog>,
}

/// O que a janela recebe do laço que acompanha o núcleo.
#[derive(Debug)]
pub enum CoreUpdate {
    Snapshot(Box<AppSnapshot>),
    /// O canal de estado fechou: o núcleo parou.
    Stopped,
}

/// Repassa cada novo estado e avisa uma única vez se o canal fechar. No
/// encerramento normal este laço é descartado antes, junto com a janela, e o
/// canal não chega a fechar; por isso `Stopped` sempre indica falha.
pub async fn forward_snapshots(
    mut state: watch::Receiver<AppSnapshot>,
    mut emit: impl FnMut(CoreUpdate),
) {
    loop {
        let snapshot = state.borrow_and_update().clone();
        emit(CoreUpdate::Snapshot(Box::new(snapshot)));
        if state.changed().await.is_err() {
            emit(CoreUpdate::Stopped);
            return;
        }
    }
}

/// Componente raiz da janela principal.
pub struct App {
    hero: Controller<HeroPage>,
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
    type CommandOutput = CoreUpdate;
    type Root = adw::ApplicationWindow;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder()
            .title(translate("Dota Beginner Companion"))
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
        let hero = HeroPage::builder()
            .launch(HeroPageInit {
                commands: core.commands.clone(),
                catalog: core.heroes,
                items: core.items,
            })
            .detach();
        let diagnostics = DiagnosticsPage::builder()
            .launch(core.commands.clone())
            .detach();

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.add_titled(hero.widget(), Some("hero"), &translate("Herói"));
        for (name, title, page) in pages::tabs() {
            stack.add_titled(&page, Some(name), &title);
        }
        stack.add_titled(
            diagnostics.widget(),
            Some("diagnostics"),
            &translate("Diagnóstico"),
        );
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

        let state = core.state;
        sender.command(move |output, shutdown| {
            shutdown
                .register(forward_snapshots(state, move |update| output.emit(update)))
                .drop_on_shutdown()
        });

        ComponentParts {
            model: App { hero, diagnostics },
            widgets: AppWidgets { connection },
        }
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        update: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match update {
            CoreUpdate::Snapshot(snapshot) => {
                widgets.connection.set_label(&connection_label(&snapshot));
                self.hero
                    .emit(HeroMessage::Snapshot(Box::new((*snapshot).clone())));
                self.diagnostics
                    .emit(DiagnosticsMessage::Snapshot(snapshot));
            }
            CoreUpdate::Stopped => {
                widgets
                    .connection
                    .set_label(&translate("GSI: núcleo parado"));
                self.hero.emit(HeroMessage::CoreStopped);
                self.diagnostics.emit(DiagnosticsMessage::CoreStopped);
            }
        }
    }
}

/// Indicador curto da barra de título. "Conectado" só aparece com eventos
/// reais do Dota recentes; o teste manual não conta.
fn connection_label(snapshot: &AppSnapshot) -> String {
    match (&snapshot.gsi.server, &snapshot.gsi.last_dota_event) {
        (ServerState::Failed(_), _) => translate("GSI: servidor com erro"),
        (ServerState::Starting, _) => translate("GSI: iniciando…"),
        (ServerState::Listening(_), None) => translate("GSI: aguardando o Dota"),
        (ServerState::Listening(_), Some(event)) => {
            let elapsed_secs = event
                .at
                .elapsed()
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or_default();
            if elapsed_secs <= 90 {
                translate("GSI: recebendo dados do Dota")
            } else {
                translate_with(
                    "GSI: sem dados há {} min",
                    &[&(elapsed_secs / 60).to_string()],
                )
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/presentation/app_tests.rs"]
mod tests;
