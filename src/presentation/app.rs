//! Janela principal: abas no topo e indicador de conexão.

use relm4::adw::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, adw, gtk,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{mpsc, watch};

use crate::application::event_bus::UiCommand;
use crate::application::state::{AppSnapshot, ServerState};
use crate::domain::ability_catalog::AbilityIdCatalog;
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
    pub abilities: Arc<AbilityIdCatalog>,
}

/// O que a janela recebe do laço que acompanha o núcleo.
#[derive(Debug, PartialEq, Eq)]
pub enum CoreUpdate {
    /// Há estado novo no canal; a janela lê o mais recente ao tratar o aviso.
    Changed,
    /// O canal de estado fechou: o núcleo parou.
    Stopped,
}

/// Marca que já existe um aviso de estado novo esperando a janela. Enquanto
/// ele não é tratado, as mudanças seguintes não geram outro aviso: a janela
/// recebe no máximo um por vez e lê sempre o estado mais recente.
#[derive(Debug, Default)]
pub struct PendingUpdate(AtomicBool);

impl PendingUpdate {
    /// Marca o aviso. Devolve `true` só se não havia aviso pendente.
    pub fn mark(&self) -> bool {
        // `AcqRel`: a troca vê a liberação feita pela janela em `release`.
        !self.0.swap(true, Ordering::AcqRel)
    }

    /// Libera o aviso. A janela chama antes de ler o estado, para que uma
    /// mudança que chegue durante a leitura produza um novo aviso.
    pub fn release(&self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Libera o aviso e devolve o estado mais recente, se esta janela ainda não o
/// leu. Um aviso pode chegar depois de a janela já ter lido a mudança que o
/// gerou; nesse caso não há nada novo e nada é repassado às páginas.
pub fn read_new_snapshot(
    state: &mut watch::Receiver<AppSnapshot>,
    pending: &PendingUpdate,
) -> Option<AppSnapshot> {
    pending.release();
    // Leitura curta: o núcleo espera só durante a cópia do estado.
    let snapshot = state.borrow_and_update();
    snapshot.has_changed().then(|| snapshot.clone())
}

/// Avisa quando há estado novo, com no máximo um aviso pendente, e avisa uma
/// única vez se o canal fechar. No encerramento normal este laço é descartado
/// antes, junto com a janela, e o canal não chega a fechar; por isso `Stopped`
/// sempre indica falha.
pub async fn forward_snapshots(
    mut state: watch::Receiver<AppSnapshot>,
    pending: Arc<PendingUpdate>,
    mut emit: impl FnMut(CoreUpdate),
) {
    // O estado inicial também é avisado.
    state.mark_changed();
    loop {
        if state.changed().await.is_err() {
            emit(CoreUpdate::Stopped);
            return;
        }
        if pending.mark() {
            emit(CoreUpdate::Changed);
        }
    }
}

/// Largura mínima da janela: a página da build não tem composição para menos
/// de 760sp (as margens laterais somam 24). Em larguras menores a página
/// travava, então a janela simplesmente não encolhe mais que isso.
const MIN_WINDOW_WIDTH: i32 = 790;
/// Componente raiz da janela principal.
pub struct App {
    hero: Controller<HeroPage>,
    diagnostics: Controller<DiagnosticsPage>,
    /// Leitura do estado mais recente quando chega `CoreUpdate::Changed`.
    state: watch::Receiver<AppSnapshot>,
    pending: Arc<PendingUpdate>,
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
            .default_width(1180)
            .default_height(860)
            .width_request(MIN_WINDOW_WIDTH)
            .height_request(400)
            .build()
    }

    fn init(
        core: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // O GTK já abriu a tela; o estilo vale para a janela inteira.
        super::style::install();
        let hero = HeroPage::builder()
            .launch(HeroPageInit {
                commands: core.commands.clone(),
                catalog: core.heroes,
                items: core.items,
                abilities: core.abilities,
            })
            .detach();
        let diagnostics = DiagnosticsPage::builder()
            .launch(core.commands.clone())
            .detach();

        // As abas ficam no topo, lado a lado, para dar a largura toda ao
        // conteúdo.
        let stack = adw::ViewStack::new();
        stack.set_hexpand(true);
        stack.add_titled_with_icon(
            hero.widget(),
            Some("hero"),
            &translate("Herói"),
            "avatar-default-symbolic",
        );
        for (name, title, page) in pages::tabs() {
            let icon = match name {
                "history" => "document-open-recent-symbolic",
                _ => "emblem-system-symbolic",
            };
            stack.add_titled_with_icon(&page, Some(name), &title, icon);
        }
        stack.add_titled_with_icon(
            diagnostics.widget(),
            Some("diagnostics"),
            &translate("Diagnóstico"),
            "utilities-system-monitor-symbolic",
        );
        // A aba Diagnóstico abre por padrão.
        stack.set_visible_child_name("diagnostics");

        let switcher = adw::ViewSwitcher::builder()
            .stack(&stack)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();

        let connection = gtk::Label::new(None);
        connection.add_css_class("dim-label");
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&switcher));
        header.pack_end(&connection);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&stack));
        root.set_content(Some(&toolbar));

        let state = core.state.clone();
        // O estado inicial também é repassado às páginas no primeiro aviso.
        let mut window_state = core.state;
        window_state.mark_changed();
        let pending = Arc::new(PendingUpdate::default());
        let forward_pending = Arc::clone(&pending);
        sender.command(move |output, shutdown| {
            shutdown
                .register(forward_snapshots(state, forward_pending, move |update| {
                    output.emit(update)
                }))
                .drop_on_shutdown()
        });

        ComponentParts {
            model: App {
                hero,
                diagnostics,
                state: window_state,
                pending,
            },
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
            CoreUpdate::Changed => {
                let Some(snapshot) = read_new_snapshot(&mut self.state, &self.pending) else {
                    return;
                };
                let snapshot = Box::new(snapshot);
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
