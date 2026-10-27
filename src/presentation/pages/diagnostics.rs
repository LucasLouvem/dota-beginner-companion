//! Tela "Diagnóstico": a saúde do aplicativo em cards (Steam, Dota 2,
//! servidor GSI, configuração GSI e recebimento de dados), com o estado
//! geral no alto, as ações explicadas e, num bloco secundário e fechado, os
//! detalhes técnicos. Só exibe o estado e envia comandos ao núcleo; o texto e
//! as regras de cada estado ficam em `diagnostics_health`.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};
use tokio::sync::mpsc::UnboundedSender;

use crate::application::error::{ToDiagnostic, log_error};
use crate::application::event_bus::{CoreConnectionError, UiCommand};
use crate::application::state::{AppSnapshot, InstallOutcome};
use crate::presentation::card_grid::{CardGrid, Columns, Layout};
use crate::presentation::i18n::translate;
use crate::presentation::widgets::{Level, RowContent, RowSection, boxed_list};

use super::diagnostics_health::{
    HealthCard, HealthContent, OverallState, TechnicalDetails, first_library, health_content,
    latest_event,
};
use super::send_command;

/// Espaço entre os cards, em pixels.
const GAP: i32 = 8;
/// Margem interna dos cards.
const CARD_PADDING: i32 = 12;
/// Até esta largura, a grade passa de três para duas colunas.
const MEDIUM_LAYOUT: &str = "max-width: 1100sp";
/// A janela não encolhe abaixo disto (ver `MIN_WINDOW_WIDTH`).
const MIN_PAGE_WIDTH: i32 = 700;
const CARD_COLUMNS: Columns = Columns::new(3, 2);

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

/// Ícone do estado, além do texto e da cor.
fn level_icon(level: Level) -> &'static str {
    match level {
        Level::Ok => "emblem-ok-symbolic",
        Level::Warning => "dialog-warning-symbolic",
        Level::Error => "dialog-error-symbolic",
        Level::Info => "dialog-information-symbolic",
    }
}

fn level_class(level: Level) -> &'static str {
    match level {
        Level::Ok => "success",
        Level::Warning => "warning",
        Level::Error => "error",
        Level::Info => "dim-label",
    }
}

/// Troca o estilo de estado de um widget só quando o nível mudou.
fn set_level(widget: &impl IsA<gtk::Widget>, shown: Option<Level>, level: Level) {
    if shown == Some(level) {
        return;
    }
    if let Some(previous) = shown {
        widget.remove_css_class(level_class(previous));
    }
    widget.add_css_class(level_class(level));
}

fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}

/// Card de um item da saúde: ícone, título, estado e uma explicação curta.
struct HealthCardWidget {
    root: gtk::Box,
    icon: gtk::Image,
    title: gtk::Label,
    state: gtk::Label,
    description: gtk::Label,
    shown: Option<HealthCard>,
}

impl HealthCardWidget {
    fn new() -> Self {
        let icon = gtk::Image::new();
        let title = gtk::Label::builder().xalign(0.0).hexpand(true).build();
        title.add_css_class("heading");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        top.append(&icon);
        top.append(&title);
        let state = gtk::Label::builder().xalign(0.0).wrap(true).build();
        state.add_css_class("title-4");
        let description = gtk::Label::builder().xalign(0.0).wrap(true).build();
        description.add_css_class("dim-label");
        description.add_css_class("caption");
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
        inner.set_margin_top(CARD_PADDING);
        inner.set_margin_bottom(CARD_PADDING);
        inner.set_margin_start(CARD_PADDING);
        inner.set_margin_end(CARD_PADDING);
        inner.append(&top);
        inner.append(&state);
        inner.append(&description);
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .accessible_role(gtk::AccessibleRole::Group)
            .hexpand(true)
            .build();
        root.add_css_class("dota-card");
        root.append(&inner);
        Self {
            root,
            icon,
            title,
            state,
            description,
            shown: None,
        }
    }

    /// Atualiza só o que mudou.
    fn show(&mut self, card: &HealthCard) {
        if self.shown.as_ref() == Some(card) {
            return;
        }
        let previous = self.shown.as_ref().map(|shown| shown.level);
        if previous != Some(card.level) {
            self.icon.set_icon_name(Some(level_icon(card.level)));
        }
        set_level(&self.icon, previous, card.level);
        set_level(&self.state, previous, card.level);
        set_label(&self.title, &card.title);
        set_label(&self.state, &card.state);
        set_label(&self.description, &card.description);
        self.root
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "{}: {}",
                card.title, card.state
            ))]);
        self.shown = Some(card.clone());
    }
}

/// Estado geral, no alto da tela.
struct OverallWidget {
    root: gtk::Box,
    icon: gtk::Image,
    title: gtk::Label,
    description: gtk::Label,
    shown: Option<OverallState>,
}

impl OverallWidget {
    fn new() -> Self {
        let icon = gtk::Image::builder().pixel_size(32).build();
        let title = gtk::Label::builder().xalign(0.0).build();
        title.add_css_class("title-2");
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let description = gtk::Label::builder().xalign(0.0).wrap(true).build();
        description.add_css_class("dim-label");
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.append(&title);
        text.append(&description);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        inner.set_margin_top(CARD_PADDING);
        inner.set_margin_bottom(CARD_PADDING);
        inner.set_margin_start(CARD_PADDING);
        inner.set_margin_end(CARD_PADDING);
        inner.append(&icon);
        inner.append(&text);
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .accessible_role(gtk::AccessibleRole::Group)
            .build();
        root.add_css_class("dota-card");
        root.update_property(&[gtk::accessible::Property::Label(&translate("Estado geral"))]);
        root.append(&inner);
        Self {
            root,
            icon,
            title,
            description,
            shown: None,
        }
    }

    fn show(&mut self, overall: OverallState) {
        if self.shown == Some(overall) {
            return;
        }
        let previous = self.shown.map(OverallState::level);
        self.icon.set_icon_name(Some(level_icon(overall.level())));
        set_level(&self.icon, previous, overall.level());
        set_level(&self.title, previous, overall.level());
        set_label(&self.title, &overall.title());
        set_label(&self.description, &overall.description());
        self.shown = Some(overall);
    }
}

/// Bloco "Detalhes técnicos", fechado por padrão.
struct DetailsWidget {
    received: gtk::Label,
    recorded: gtk::Label,
    rejected: gtk::Label,
    log: RowSection,
    log_list: gtk::ListBox,
    config_path: gtk::Label,
    json_source: gtk::Label,
    json_buffer: gtk::TextBuffer,
}

impl DetailsWidget {
    fn new() -> (Self, gtk::Expander) {
        let small = || {
            let label = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .selectable(true)
                .build();
            label.add_css_class("dim-label");
            label.add_css_class("caption");
            label
        };
        let (received, recorded, rejected, config_path, json_source) =
            (small(), small(), small(), small(), small());
        let log_list = boxed_list();
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
            .min_content_height(200)
            .child(&json_view)
            .build();
        json_scroll.add_css_class("card");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        for widget in [
            received.upcast_ref::<gtk::Widget>(),
            recorded.upcast_ref(),
            rejected.upcast_ref(),
            log_list.upcast_ref(),
            config_path.upcast_ref(),
            json_source.upcast_ref(),
            json_scroll.upcast_ref(),
        ] {
            body.append(widget);
        }
        let expander = gtk::Expander::new(Some(&translate("Detalhes técnicos")));
        expander.set_child(Some(&body));
        (
            Self {
                received,
                recorded,
                rejected,
                log: RowSection::new(log_list.clone()),
                log_list,
                config_path,
                json_source,
                json_buffer,
            },
            expander,
        )
    }

    fn show(&mut self, details: &TechnicalDetails) {
        set_label(&self.received, &details.received);
        let optional = |label: &gtk::Label, text: &Option<String>| {
            set_label(label, text.as_deref().unwrap_or(""));
            label.set_visible(text.is_some());
        };
        optional(&self.recorded, &details.recorded);
        optional(&self.rejected, &details.rejected);
        self.log.show(details.log.iter().cloned().collect());
        self.log_list.set_visible(details.log.is_some());
        set_label(&self.config_path, &details.config_path);
        set_label(&self.json_source, &details.json_source);
    }
}

/// Widgets da página Diagnóstico.
pub struct DiagnosticsWidgets {
    overall: OverallWidget,
    cards: Vec<HealthCardWidget>,
    install_button: gtk::Button,
    results: RowSection,
    results_list: gtk::ListBox,
    details: DetailsWidget,
}

/// Botão com um texto que explica o resultado esperado.
fn action(button: &gtk::Button, hint: &str) -> gtk::Box {
    let caption = gtk::Label::builder()
        .label(hint)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(34)
        .build();
    caption.add_css_class("dim-label");
    caption.add_css_class("caption");
    button.set_halign(gtk::Align::Start);
    button.update_property(&[gtk::accessible::Property::Description(hint)]);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    column.append(button);
    column.append(&caption);
    column
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
        let content = gtk::Box::new(gtk::Orientation::Vertical, GAP);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);

        // Cabeçalho: título, explicação e o estado geral.
        let title = gtk::Label::new(Some(&translate("Diagnóstico")));
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        title.set_accessible_role(gtk::AccessibleRole::Heading);
        let intro = gtk::Label::new(Some(&translate(
            "Verifica se o aplicativo está pronto para acompanhar o Dota 2. Nada aqui altera o jogo.",
        )));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");
        let overall = OverallWidget::new();
        content.append(&title);
        content.append(&intro);
        content.append(&overall.root);

        // Cards de saúde, em grade.
        let grid = CardGrid::new(GAP, CARD_COLUMNS);
        let cards: Vec<HealthCardWidget> = (0..5).map(|_| HealthCardWidget::new()).collect();
        for card in &cards {
            grid.push(&card.root, 1);
        }
        content.append(grid.widget());

        // Ações, cada uma com a explicação do que faz.
        let run_button = gtk::Button::with_label(&translate("Rodar diagnóstico novamente"));
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
        let actions = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .max_children_per_line(3)
            .row_spacing(GAP as u32)
            .column_spacing(16)
            .halign(gtk::Align::Start)
            .build();
        for (button, hint) in [
            (
                &install_button,
                translate(
                    "Cria ou atualiza o arquivo que faz o Dota enviar dados ao aplicativo. Reinicie o Dota depois.",
                ),
            ),
            (
                &test_button,
                translate(
                    "Envia um evento de teste ao servidor local. Não confirma que o Dota está enviando dados.",
                ),
            ),
            (
                &run_button,
                translate("Refaz todas as verificações desta tela."),
            ),
        ] {
            let child = gtk::FlowBoxChild::new();
            child.set_focusable(false);
            child.set_child(Some(&action(button, &hint)));
            actions.append(&child);
        }
        content.append(&actions);

        // Resultado das ações e falhas.
        let results_list = boxed_list();
        results_list.set_visible(false);
        content.append(&results_list);

        // Detalhes técnicos, fechados.
        let (details, expander) = DetailsWidget::new();
        content.append(&expander);

        // As cards mudam de colunas pelo sinal `apply` do ponto de quebra.
        let bin = adw::BreakpointBin::builder()
            .width_request(MIN_PAGE_WIDTH)
            .height_request(200)
            .child(&content)
            .build();
        let medium = adw::Breakpoint::new(
            adw::BreakpointCondition::parse(MEDIUM_LAYOUT)
                .expect("a condição de largura é uma constante válida"),
        );
        let applied = Rc::new(Cell::new(false));
        for medium_applied in [true, false] {
            let (grid, applied) = (grid.clone(), Rc::clone(&applied));
            let update = move |_: &adw::Breakpoint| {
                applied.set(medium_applied);
                grid.set_layout(if medium_applied {
                    Layout::Medium
                } else {
                    Layout::Wide
                });
            };
            if medium_applied {
                medium.connect_apply(update);
            } else {
                medium.connect_unapply(update);
            }
        }
        bin.add_breakpoint(medium);
        // O `BreakpointBin` informa altura mínima zero; com a política padrão
        // do `Viewport` (mínima), a página cortaria o conteúdo. A política
        // natural a faz rolar.
        let viewport = gtk::Viewport::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        viewport.set_vscroll_policy(gtk::ScrollablePolicy::Natural);
        viewport.set_child(Some(&bin));
        root.set_child(Some(&viewport));

        let model = DiagnosticsPage {
            commands,
            snapshot: AppSnapshot::default(),
            handled_install: 0,
            shown_raw: None,
            core_failure: None,
        };
        let mut widgets = DiagnosticsWidgets {
            overall,
            cards,
            install_button,
            results: RowSection::new(results_list.clone()),
            results_list,
            details,
        };
        let mut model = model;
        model.render(&mut widgets);
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
                if let Some(root) = first_library(&self.snapshot) {
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
                if let Some(root) = first_library(&self.snapshot) {
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

    /// Atualiza só o que mudou desde o último estado exibido.
    fn render(&mut self, widgets: &mut DiagnosticsWidgets) {
        let snapshot = &self.snapshot;
        let core_failure = self
            .core_failure
            .as_ref()
            .map(|core_error| core_error.to_diagnostic(&snapshot.redactor));
        let content: HealthContent = health_content(snapshot, core_failure.as_ref());

        widgets.overall.show(content.overall);
        for (widget, card) in widgets.cards.iter_mut().zip(&content.cards) {
            widget.show(card);
        }
        if widgets.install_button.is_sensitive() != content.can_install {
            widgets.install_button.set_sensitive(content.can_install);
        }
        let results: Vec<RowContent> = content.results;
        widgets.results_list.set_visible(!results.is_empty());
        widgets.results.show(results);
        widgets.details.show(&content.details);

        let raw = latest_event(snapshot).map(|(event, _)| Arc::clone(&event.raw));
        if raw.as_ref().map(Arc::as_ptr) != self.shown_raw.as_ref().map(Arc::as_ptr) {
            widgets
                .details
                .json_buffer
                .set_text(raw.as_deref().unwrap_or(""));
            self.shown_raw = raw;
        }
    }
}

fn confirm_overwrite(
    root: &gtk::ScrolledWindow,
    path: PathBuf,
    sender: ComponentSender<DiagnosticsPage>,
) {
    let dialog = adw::AlertDialog::new(
        Some(&translate("Substituir arquivo GSI existente?")),
        Some(&crate::presentation::i18n::translate_with(
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
