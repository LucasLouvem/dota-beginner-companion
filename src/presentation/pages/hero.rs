//! Tela "Herói": a página da build (vitórias recentes para escolher, com os
//! itens e a ordem de habilidades acompanhados pela partida), o herói do
//! jogador com os dados embutidos, as estatísticas, os itens e as
//! habilidades que o GSI informa e a linha do tempo da partida. Sem herói na
//! partida atual, mostra a última partida terminada. Cada informação diz de
//! onde vem: dado embutido, confirmado pelo GSI, inferido ou marcado pelo
//! usuário. Quando uma partida é encontrada, pergunta o modo de jogo numa
//! janela sobre o aplicativo.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};
use tokio::sync::mpsc::UnboundedSender;

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, log_error};
use crate::application::event_bus::{CoreConnectionError, UiCommand};
use crate::application::state::{
    AppSnapshot, MatchStatus, ModeOrigin, ModeState, RecommendationStatus, TimelineEntry,
};
use crate::application::use_cases::item_icons::ItemIcons;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::application::use_cases::recommendations::{BuildId, Recommendations};
use crate::domain::ability_catalog::AbilityIdCatalog;
use crate::domain::game::{CarriedItem, GameMode, GamePhase, GameSnapshot, HeroPick, PlayerStats};
use crate::domain::hero_catalog::{AttackType, HeroCatalog, HeroInfo, HeroRole, PrimaryAttribute};
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_events::{
    ItemPlacement, MatchEventKind, MatchResult, MatchSummary, Provenance,
};
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{
    Level, RowContent, RowSection, boxed_list, format_duration, format_time, section_title,
    status_row,
};

use super::build::{
    BuildCatalogs, SidebarItem, build_detail, entry_at, header_rows, index_of, popularity_rows,
    progress_row, sidebar, sidebar_items,
};
use super::build_panel::{BuildPanel, BuildView};
use super::send_command;

/// De onde vem a lista de habilidades exibida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilitySource {
    /// Habilidades e níveis enviados pelo GSI na partida.
    Gsi,
    /// Lista dos dados embutidos, sem níveis; o GSI ainda não enviou habilidades.
    Embedded,
}

/// Habilidade pronta para exibir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityLine {
    pub name: String,
    pub level: Option<u32>,
    pub ultimate: bool,
}

/// O que a aba mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroView<'a> {
    /// O estado da partida atual (ou nenhum, antes do primeiro evento).
    Current(Option<&'a GameSnapshot>),
    /// A última partida terminada, enquanto a partida atual não tem herói.
    LastMatch(&'a MatchSummary),
}

/// Conteúdo das seções da aba, exceto a linha do tempo, na ordem exibida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroContent {
    pub mode: Vec<RowContent>,
    /// Título da seção do herói: "Herói escolhido" ou "Última partida".
    pub title: String,
    pub hero: Vec<RowContent>,
    pub stats: Vec<RowContent>,
    pub items: Vec<RowContent>,
    pub abilities: Vec<RowContent>,
}

/// Alteração na linha do tempo: eventos mais antigos que saíram e eventos
/// novos que entraram, do mais antigo ao mais recente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineChange<'a> {
    pub dropped_oldest: usize,
    pub appended: &'a [TimelineEntry],
}

impl TimelineChange<'_> {
    pub fn is_empty(&self) -> bool {
        self.dropped_oldest == 0 && self.appended.is_empty()
    }
}

/// Por que a pergunta do modo foi aberta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeDialogKind {
    /// Uma partida foi encontrada; fechar sem confirmar vale o pré-selecionado.
    MatchFound,
    /// O usuário quer alterar o modo já informado; fechar não muda nada.
    Change,
}

/// Página Herói.
pub struct HeroPage {
    commands: UnboundedSender<UiCommand>,
    catalog: Arc<HeroCatalog>,
    items: Arc<ItemCatalog>,
    abilities: Arc<AbilityIdCatalog>,
    match_status: MatchStatus,
    recommendations: RecommendationStatus,
    item_icons: Arc<ItemIcons>,
    /// Build escolhida, com a consulta em que foi escolhida. Muda só pela
    /// escolha na lista ou quando a consulta muda; nunca vai para o núcleo.
    selection: Option<(RecommendationKey, BuildId)>,
    /// Linhas da lista lateral e as recomendações de onde vieram, para não
    /// refazer a lista a cada evento do GSI.
    sidebar: Vec<SidebarItem>,
    sidebar_source: Option<Arc<Recommendations>>,
    redactor: Redactor,
    /// Falha de comunicação com o núcleo, mostrada na própria página.
    core_failure: Option<CoreConnectionError>,
    /// Última partida cuja pergunta do modo já foi aberta.
    handled_match: u64,
    /// Pergunta do modo aberta para uma partida encontrada, com o número dela.
    /// É fechada se outra partida for encontrada antes da resposta.
    open_mode_dialog: Option<(u64, adw::AlertDialog)>,
}

/// Mensagens recebidas pela página Herói.
#[derive(Debug)]
pub enum HeroMessage {
    Snapshot(Box<AppSnapshot>),
    CoreStopped,
    MarkMatchFound,
    /// Botão "Alterar modo de jogo".
    ChangeMode,
    /// Linha escolhida na lista de vitórias.
    SelectBuildRow(i32),
    /// Resposta da pergunta do modo para a partida `match_number`.
    ChooseMode {
        match_number: u64,
        mode: GameMode,
        origin: ModeOrigin,
    },
}

/// Dados para iniciar a página.
pub struct HeroPageInit {
    pub commands: UnboundedSender<UiCommand>,
    pub catalog: Arc<HeroCatalog>,
    pub items: Arc<ItemCatalog>,
    pub abilities: Arc<AbilityIdCatalog>,
}

/// Widgets da página, com o que cada lista mostra agora.
pub struct HeroWidgets {
    mode: RowSection,
    change_mode_button: gtk::Button,
    build: BuildPanel,
    hero_title: gtk::Label,
    hero: RowSection,
    stats: RowSection,
    items: RowSection,
    abilities: RowSection,
    timeline: TimelineSection,
}

/// Linha do tempo com o evento mais recente em cima. Eventos novos entram no
/// topo e os mais antigos saem de baixo, sem recriar as outras linhas.
struct TimelineSection {
    list: gtk::ListBox,
    /// Eventos exibidos, do mais antigo ao mais recente, como em `MatchStatus`.
    entries: Vec<TimelineEntry>,
    /// Linha e subtítulo exibido de cada evento, na mesma ordem de `entries`.
    rows: Vec<(adw::ActionRow, String)>,
    /// Linha "Nenhum evento da partida ainda", presente com a lista vazia.
    placeholder: Option<adw::ActionRow>,
}

impl TimelineSection {
    fn new(list: gtk::ListBox) -> Self {
        TimelineSection {
            list,
            entries: Vec::new(),
            rows: Vec::new(),
            placeholder: None,
        }
    }

    fn show(&mut self, timeline: &[TimelineEntry], catalog: &HeroCatalog, items: &ItemCatalog) {
        let change = timeline_changes(&self.entries, timeline);
        for (row, _) in self.rows.drain(..change.dropped_oldest) {
            self.list.remove(&row);
        }
        // O subtítulo tem o tempo relativo ("há 5 s"), que muda sem novo evento.
        for ((row, shown_subtitle), entry) in self.rows.iter_mut().zip(timeline) {
            let subtitle = timeline_subtitle(entry);
            if *shown_subtitle != subtitle {
                row.set_subtitle(&subtitle);
                *shown_subtitle = subtitle;
            }
        }
        for entry in change.appended {
            let subtitle = timeline_subtitle(entry);
            let row = status_row(
                Level::Info,
                &event_label(&entry.event.kind, catalog, items),
                &subtitle,
            );
            self.list.prepend(&row);
            self.rows.push((row, subtitle));
        }
        if !change.is_empty() {
            self.entries.drain(..change.dropped_oldest);
            self.entries.extend_from_slice(change.appended);
        }

        match (&self.placeholder, timeline.is_empty()) {
            (None, true) => {
                let placeholder = status_row(
                    Level::Info,
                    &translate("Nenhum evento da partida ainda"),
                    &translate("Os eventos aparecem quando o Dota envia dados de uma partida."),
                );
                self.list.append(&placeholder);
                self.placeholder = Some(placeholder);
            }
            (Some(placeholder), false) => {
                self.list.remove(placeholder);
                self.placeholder = None;
            }
            _ => {}
        }
    }
}

impl Component for HeroPage {
    type Init = HeroPageInit;
    type Input = HeroMessage;
    type Output = ();
    type CommandOutput = ();
    type Root = gtk::ScrolledWindow;
    type Widgets = HeroWidgets;

    fn init_root() -> Self::Root {
        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build()
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(16);
        content.set_margin_end(16);

        let title = gtk::Label::new(Some(&translate("Herói")));
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let intro = gtk::Label::new(Some(&translate(
            "Escolha uma vitória recente com o herói para acompanhar a build dela durante a \
             partida: itens e ordem de habilidades, com o que você já comprou e aprendeu. \
             Abaixo ficam o herói, as estatísticas, os itens, as habilidades e o que \
             aconteceu na partida; depois que ela termina, o resumo fica até a próxima ser \
             encontrada. Nada é enviado ao jogo.",
        )));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");
        content.append(&title);
        content.append(&intro);

        let match_found_button = gtk::Button::with_label(&translate("Partida encontrada"));
        match_found_button.set_halign(gtk::Align::Start);
        match_found_button.set_tooltip_text(Some(&translate(
            "Marca manualmente que uma partida foi encontrada. O GSI não informa esse momento; \
             o aplicativo também o deduz quando a seleção de heróis começa.",
        )));
        {
            let button_sender = sender.clone();
            match_found_button
                .connect_clicked(move |_| button_sender.input(HeroMessage::MarkMatchFound));
        }
        content.append(&match_found_button);

        let mode_list = boxed_list();
        content.append(&section_title(&translate("Modo de jogo")));
        content.append(&mode_list);
        let change_mode_button = gtk::Button::with_label(&translate("Alterar modo de jogo"));
        change_mode_button.set_halign(gtk::Align::Start);
        change_mode_button.set_sensitive(false);
        {
            let button_sender = sender.clone();
            change_mode_button
                .connect_clicked(move |_| button_sender.input(HeroMessage::ChangeMode));
        }
        content.append(&change_mode_button);

        let build = {
            let select_sender = sender.clone();
            BuildPanel::new(move |index| select_sender.input(HeroMessage::SelectBuildRow(index)))
        };
        content.append(&section_title(&translate("Build")));
        content.append(build.widget());

        let hero_title = section_title("");
        let hero_list = boxed_list();
        content.append(&hero_title);
        content.append(&hero_list);

        let stats_list = boxed_list();
        content.append(&section_title(&translate("Estatísticas")));
        content.append(&stats_list);

        let items_list = boxed_list();
        content.append(&section_title(&translate("Itens")));
        content.append(&items_list);

        let ability_list = boxed_list();
        content.append(&section_title(&translate("Habilidades")));
        content.append(&ability_list);

        let timeline_list = boxed_list();
        content.append(&section_title(&translate("Linha do tempo da partida")));
        content.append(&timeline_list);

        let clamp = adw::Clamp::builder()
            .maximum_size(1200)
            .child(&content)
            .build();
        root.set_child(Some(&clamp));

        let model = HeroPage {
            commands: init.commands,
            catalog: init.catalog,
            items: init.items,
            abilities: init.abilities,
            match_status: MatchStatus::default(),
            recommendations: RecommendationStatus::default(),
            item_icons: Arc::default(),
            selection: None,
            sidebar: Vec::new(),
            sidebar_source: None,
            redactor: Redactor::default(),
            core_failure: None,
            handled_match: 0,
            open_mode_dialog: None,
        };
        let mut widgets = HeroWidgets {
            mode: RowSection::new(mode_list),
            change_mode_button,
            build,
            hero_title,
            hero: RowSection::new(hero_list),
            stats: RowSection::new(stats_list),
            items: RowSection::new(items_list),
            abilities: RowSection::new(ability_list),
            timeline: TimelineSection::new(timeline_list),
        };
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
            HeroMessage::Snapshot(snapshot) => {
                self.match_status = snapshot.match_status;
                self.redactor = snapshot.redactor;
                self.item_icons = snapshot.item_icons;
                self.update_recommendations(snapshot.recommendations);
                if let Some((match_number, preselected)) =
                    mode_dialog_request(&self.match_status, self.handled_match)
                {
                    self.handled_match = match_number;
                    // A pergunta de uma partida anterior não vale mais; a
                    // resposta dela, se vier, é descartada pelo núcleo.
                    if let Some((_, stale_dialog)) = self.open_mode_dialog.take() {
                        stale_dialog.force_close();
                    }
                    let dialog = present_mode_dialog(
                        root,
                        ModeDialogKind::MatchFound,
                        match_number,
                        preselected,
                        sender,
                    );
                    self.open_mode_dialog = Some((match_number, dialog));
                }
            }
            HeroMessage::CoreStopped => self.report_core_failure(CoreConnectionError::StateClosed),
            HeroMessage::MarkMatchFound => self.send(UiCommand::MarkMatchFound),
            HeroMessage::SelectBuildRow(index) => {
                if let (RecommendationStatus::Ready(recommendations), Some(id)) =
                    (&self.recommendations, entry_at(&self.sidebar, index))
                {
                    self.selection = Some((recommendations.key.clone(), id));
                }
            }
            HeroMessage::ChangeMode => {
                if let Some(current) = change_mode_preselection(&self.match_status) {
                    present_mode_dialog(
                        root,
                        ModeDialogKind::Change,
                        self.match_status.match_number,
                        current,
                        sender,
                    );
                }
            }
            HeroMessage::ChooseMode {
                match_number,
                mode,
                origin,
            } => {
                if self
                    .open_mode_dialog
                    .as_ref()
                    .is_some_and(|(open_match, _)| *open_match == match_number)
                {
                    self.open_mode_dialog = None;
                }
                self.send(UiCommand::ChooseGameMode {
                    match_number,
                    mode,
                    origin,
                });
            }
        }
        self.render(widgets);
    }
}

impl HeroPage {
    /// Guarda as recomendações novas: refaz a lista lateral só quando elas
    /// mudam e mantém a build escolhida enquanto a consulta for a mesma.
    fn update_recommendations(&mut self, status: RecommendationStatus) {
        match &status {
            RecommendationStatus::Ready(recommendations) => {
                let changed = !self
                    .sidebar_source
                    .as_ref()
                    .is_some_and(|source| Arc::ptr_eq(source, recommendations));
                if changed {
                    self.sidebar = sidebar_items(&sidebar(&status, &self.items));
                    self.sidebar_source = Some(Arc::clone(recommendations));
                }
                self.selection = recommendations
                    .resolve_selection(self.selection.as_ref())
                    .map(|id| (recommendations.key.clone(), id));
            }
            _ => {
                self.sidebar = sidebar_items(&sidebar(&status, &self.items));
                self.sidebar_source = None;
            }
        }
        self.recommendations = status;
    }

    fn send(&mut self, command: UiCommand) {
        if let Err(core_error) = send_command(&self.commands, command) {
            self.report_core_failure(core_error);
        }
    }

    /// Guarda a primeira falha de comunicação com o núcleo.
    fn report_core_failure(&mut self, core_error: CoreConnectionError) {
        if self.core_failure.is_none() {
            log_error(
                &core_error.to_diagnostic(&self.redactor),
                "user interface lost the connection to the core",
            );
            self.core_failure = Some(core_error);
        }
    }

    /// Página da build: cabeçalho e lista não dependem da build escolhida;
    /// a build é acompanhada pelo estado atual da partida.
    fn render_build(&self, panel: &mut BuildPanel) {
        let game = self.match_status.game.as_ref();
        let catalogs = BuildCatalogs {
            heroes: &self.catalog,
            items: &self.items,
            abilities: &self.abilities,
        };
        let (detail, popularity) = match &self.recommendations {
            RecommendationStatus::Ready(recommendations) => (
                self.selection
                    .as_ref()
                    .and_then(|(_, id)| build_detail(recommendations, id, game, &catalogs)),
                popularity_rows(recommendations, &self.items, &self.redactor),
            ),
            _ => (None, Vec::new()),
        };
        let selected = self
            .selection
            .as_ref()
            .and_then(|(_, id)| index_of(&self.sidebar, id));
        panel.show(&BuildView {
            header: header_rows(
                &self.recommendations,
                &self.match_status.mode,
                &self.catalog,
                &self.redactor,
            ),
            sidebar: &self.sidebar,
            selected,
            detail: detail.as_ref(),
            progress: progress_row(game),
            popularity,
            icons: &self.item_icons,
        });
    }

    /// Atualiza só as linhas que mudaram desde o último estado exibido.
    fn render(&self, widgets: &mut HeroWidgets) {
        let core_failure = self
            .core_failure
            .as_ref()
            .map(|core_error| core_error.to_diagnostic(&self.redactor));
        let content = hero_content(
            &self.match_status,
            core_failure.as_ref(),
            &self.catalog,
            &self.items,
        );
        widgets.mode.show(content.mode);
        self.render_build(&mut widgets.build);
        let can_change_mode = change_mode_preselection(&self.match_status).is_some();
        if widgets.change_mode_button.is_sensitive() != can_change_mode {
            widgets.change_mode_button.set_sensitive(can_change_mode);
        }
        if widgets.hero_title.label().as_str() != content.title {
            widgets.hero_title.set_label(&content.title);
        }
        widgets.hero.show(content.hero);
        widgets.stats.show(content.stats);
        widgets.items.show(content.items);
        widgets.abilities.show(content.abilities);
        widgets
            .timeline
            .show(&self.match_status.timeline, &self.catalog, &self.items);
    }
}

/// Conteúdo das seções para o estado da partida, com a falha de comunicação
/// com o núcleo, quando existe, como primeira linha do herói.
pub fn hero_content(
    status: &MatchStatus,
    core_failure: Option<&Diagnostic>,
    catalog: &HeroCatalog,
    items: &ItemCatalog,
) -> HeroContent {
    let last_match_state;
    let (game, last_match) = match hero_view(status) {
        HeroView::Current(game) => (game, None),
        HeroView::LastMatch(summary) => {
            last_match_state = last_match_snapshot(summary);
            (Some(&last_match_state), Some(summary))
        }
    };
    let hero = game.and_then(|game| picked_hero(game, catalog));
    let confirmed = translate("Confirmado pelo GSI");

    let title = if last_match.is_some() {
        translate("Última partida")
    } else {
        translate("Herói escolhido")
    };

    let mut hero_section: Vec<RowContent> = core_failure
        .map(RowContent::diagnostic)
        .into_iter()
        .collect();
    hero_section.extend(
        last_match
            .map(result_lines)
            .unwrap_or_default()
            .iter()
            .map(|line| RowContent::new(Level::Ok, line, &confirmed)),
    );
    hero_section.extend(hero_rows(game, hero));

    let stats = match game
        .map(|game| stat_lines(&game.stats, game.hero_level))
        .filter(|lines| !lines.is_empty())
    {
        None => vec![RowContent::new(
            Level::Info,
            &translate("Sem estatísticas ainda"),
            &translate(
                "Os números aparecem quando o Dota os envia, a partir da seleção de heróis.",
            ),
        )],
        Some(lines) => lines
            .iter()
            .map(|line| RowContent::new(Level::Info, line, &confirmed))
            .collect(),
    };

    let item_rows = match game.and_then(|game| game.inventory.as_deref()) {
        None => vec![RowContent::new(
            Level::Info,
            &translate("Sem itens para mostrar"),
            &translate("O Dota envia os itens a partir do pré-jogo."),
        )],
        Some([]) => vec![RowContent::new(
            Level::Info,
            &translate("Nenhum item"),
            &confirmed,
        )],
        Some(carried) => item_lines(carried, items)
            .iter()
            .map(|(name, place)| RowContent::new(Level::Info, name, place))
            .collect(),
    };

    HeroContent {
        mode: vec![mode_row(&status.mode)],
        title,
        hero: hero_section,
        stats,
        items: item_rows,
        abilities: ability_rows(game, hero),
    }
}

/// Compara a linha do tempo exibida com a nova. A linha do tempo só ganha
/// eventos no fim e perde os mais antigos (ou é reiniciada numa partida nova),
/// então o fim da exibida que coincide com o começo da nova é mantido. Sem
/// coincidência, todos os eventos saem e todos entram.
pub fn timeline_changes<'a>(
    shown: &[TimelineEntry],
    new: &'a [TimelineEntry],
) -> TimelineChange<'a> {
    // Com todos os exibidos removidos, o trecho mantido é vazio e sempre
    // coincide; por isso a busca sempre encontra um valor.
    let dropped_oldest = (0..=shown.len())
        .find(|&dropped| new.starts_with(&shown[dropped..]))
        .unwrap_or(shown.len());
    let kept = shown.len() - dropped_oldest;
    TimelineChange {
        dropped_oldest,
        appended: &new[kept..],
    }
}

/// Abre a pergunta do modo sobre a janela do aplicativo, com `preselected`
/// marcado. O sinal `response` é emitido uma vez ao fechar, pelo botão
/// "Confirmar" ou por Esc e pelo "X" (`close-response`); a trava garante uma
/// única resposta mesmo se o fechamento for forçado.
fn present_mode_dialog(
    root: &gtk::ScrolledWindow,
    kind: ModeDialogKind,
    match_number: u64,
    preselected: GameMode,
    sender: ComponentSender<HeroPage>,
) -> adw::AlertDialog {
    let body = match kind {
        ModeDialogKind::MatchFound => translate(
            "O Dota não informa o modo da partida. Fechar sem confirmar mantém o modo marcado \
             ao abrir.",
        ),
        ModeDialogKind::Change => translate("Fechar sem confirmar não altera o modo informado."),
    };
    let dialog = adw::AlertDialog::new(
        Some(&translate("Qual o modo de jogo iniciado?")),
        Some(&body),
    );
    let options = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let mut buttons: Vec<(GameMode, gtk::CheckButton)> = Vec::new();
    for mode in GameMode::ALL {
        let button = gtk::CheckButton::with_label(&mode_label(mode));
        if let Some((_, first)) = buttons.first() {
            button.set_group(Some(first));
        }
        button.set_active(mode == preselected);
        options.append(&button);
        buttons.push((mode, button));
    }
    dialog.set_extra_child(Some(&options));
    dialog.add_response(CONFIRM_RESPONSE, &translate("Confirmar"));
    dialog.set_response_appearance(CONFIRM_RESPONSE, adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some(CONFIRM_RESPONSE));
    dialog.set_close_response(CLOSE_RESPONSE);

    let answered = Rc::new(Cell::new(false));
    dialog.connect_response(None, move |_, response| {
        if answered.replace(true) {
            return;
        }
        let selected = buttons
            .iter()
            .find(|(_, button)| button.is_active())
            .map_or(preselected, |(mode, _)| *mode);
        let confirmed = response == CONFIRM_RESPONSE;
        if let Some((mode, origin)) = mode_dialog_outcome(kind, confirmed, preselected, selected) {
            sender.input(HeroMessage::ChooseMode {
                match_number,
                mode,
                origin,
            });
        }
    });
    dialog.present(Some(root));
    dialog
}

/// Resposta do botão "Confirmar" na pergunta do modo.
const CONFIRM_RESPONSE: &str = "confirm";
/// Resposta de Esc e do "X" na pergunta do modo.
const CLOSE_RESPONSE: &str = "close";

/// Partida cuja pergunta do modo ainda não foi aberta, com o modo
/// pré-selecionado. Cada partida encontrada abre a pergunta uma vez só.
pub fn mode_dialog_request(status: &MatchStatus, handled_match: u64) -> Option<(u64, GameMode)> {
    match status.mode {
        ModeState::Awaiting { preselected } if status.match_number > handled_match => {
            Some((status.match_number, preselected))
        }
        _ => None,
    }
}

/// Modo e origem a gravar quando a pergunta fecha. Só "Confirmar" é escolha
/// explícita; fechar a pergunta da partida encontrada vale o modo marcado ao
/// abrir, descartando mudanças não confirmadas, e fechar a alteração não muda
/// nada.
pub fn mode_dialog_outcome(
    kind: ModeDialogKind,
    confirmed: bool,
    preselected: GameMode,
    selected: GameMode,
) -> Option<(GameMode, ModeOrigin)> {
    match (confirmed, kind) {
        (true, _) => Some((selected, ModeOrigin::Chosen)),
        (false, ModeDialogKind::MatchFound) => Some((preselected, ModeOrigin::Default)),
        (false, ModeDialogKind::Change) => None,
    }
}

/// Modo a pré-selecionar ao alterar; só há o que alterar depois que o modo da
/// partida foi informado.
pub fn change_mode_preselection(status: &MatchStatus) -> Option<GameMode> {
    match status.mode {
        ModeState::Confirmed { mode, .. } => Some(mode),
        ModeState::NotAsked | ModeState::Awaiting { .. } => None,
    }
}

/// Linha do modo da partida, com a origem.
pub fn mode_row(mode: &ModeState) -> RowContent {
    match mode {
        ModeState::NotAsked => RowContent::new(
            Level::Info,
            &translate("Modo de jogo ainda não informado"),
            &translate("A pergunta aparece quando uma partida é encontrada."),
        ),
        ModeState::Awaiting { .. } => RowContent::new(
            Level::Warning,
            &translate("Modo de jogo aguardando confirmação"),
            &translate("Responda à pergunta aberta sobre o modo da partida."),
        ),
        ModeState::Confirmed { mode, origin } => RowContent::new(
            Level::Info,
            &translate_with("Modo de jogo: {}", &[&mode_label(*mode)]),
            &mode_origin_label(*origin),
        ),
    }
}

pub fn mode_label(mode: GameMode) -> String {
    match mode {
        GameMode::AllPick => translate("Escolha Livre"),
        GameMode::Turbo => translate("Turbo"),
        GameMode::Bots => translate("Contra bots"),
    }
}

pub fn mode_origin_label(origin: ModeOrigin) -> String {
    match origin {
        ModeOrigin::Chosen => translate("Informado por você"),
        ModeOrigin::Default => translate("Modo padrão"),
    }
}

/// A partida atual quando ela tem herói escolhido; sem herói nela, a última
/// partida terminada (que deixa de existir quando a próxima é encontrada).
pub fn hero_view(status: &MatchStatus) -> HeroView<'_> {
    let current = status.game.as_ref();
    let current_has_hero = current.is_some_and(|game| matches!(game.hero, HeroPick::Picked { .. }));
    match &status.last_match {
        Some(summary) if !current_has_hero => HeroView::LastMatch(summary),
        _ => HeroView::Current(current),
    }
}

/// Estado equivalente ao fim da última partida, para exibir o resumo com as
/// mesmas linhas de herói, estatísticas, itens e habilidades da partida atual.
pub fn last_match_snapshot(summary: &MatchSummary) -> GameSnapshot {
    GameSnapshot {
        phase: Some(GamePhase::PostGame),
        hero: summary.hero.clone(),
        hero_level: summary.hero_level,
        stats: summary.stats.clone(),
        abilities: Some(summary.abilities.clone()),
        inventory: Some(summary.final_items.clone()),
        ..GameSnapshot::menu()
    }
}

/// Resultado, duração e ID da partida terminada; linhas sem dado ficam de fora.
pub fn result_lines(summary: &MatchSummary) -> Vec<String> {
    let mut lines = vec![match summary.result {
        MatchResult::Victory => translate("Resultado: vitória"),
        MatchResult::Defeat => translate("Resultado: derrota"),
        MatchResult::Unknown => translate("Resultado não informado pelo Dota"),
    }];
    if let Some(duration_secs) = summary.duration_secs {
        lines.push(translate_with(
            "Duração: {}",
            &[&format_duration(duration_secs)],
        ));
    }
    if let Some(match_id) = &summary.match_id {
        lines.push(translate_with("ID da partida: {}", &[match_id.as_str()]));
    }
    lines
}

/// Estatísticas do jogador; cada linha só aparece com todos os seus números.
pub fn stat_lines(stats: &PlayerStats, hero_level: Option<u32>) -> Vec<String> {
    let mut lines = Vec::new();
    if let (Some(kills), Some(deaths), Some(assists)) = (stats.kills, stats.deaths, stats.assists) {
        lines.push(translate_with(
            "Abates / mortes / assistências: {} / {} / {}",
            &[
                &kills.to_string(),
                &deaths.to_string(),
                &assists.to_string(),
            ],
        ));
    }
    if let (Some(last_hits), Some(denies)) = (stats.last_hits, stats.denies) {
        lines.push(translate_with(
            "Last hits / denies: {} / {}",
            &[&last_hits.to_string(), &denies.to_string()],
        ));
    }
    if let Some(gpm) = stats.gpm {
        lines.push(translate_with(
            "Ouro por minuto (GPM): {}",
            &[&gpm.to_string()],
        ));
    }
    if let Some(xpm) = stats.xpm {
        lines.push(translate_with(
            "Experiência por minuto (XPM): {}",
            &[&xpm.to_string()],
        ));
    }
    if let Some(level) = hero_level {
        lines.push(translate_with("Nível: {}", &[&level.to_string()]));
    }
    if let Some(gold) = stats.gold {
        lines.push(translate_with("Ouro: {}", &[&gold.to_string()]));
    }
    lines
}

/// Itens na ordem dos espaços, com o nome de exibição e o lugar onde estão.
pub fn item_lines(items: &[CarriedItem], catalog: &ItemCatalog) -> Vec<(String, String)> {
    items
        .iter()
        .map(|item| {
            (
                catalog.display_name(&item.name).to_owned(),
                slot_label(&item.slot),
            )
        })
        .collect()
}

/// Lugar de um espaço do inventário do GSI.
pub fn slot_label(slot: &str) -> String {
    match slot {
        "slot0" | "slot1" | "slot2" | "slot3" | "slot4" | "slot5" => translate("Inventário"),
        "slot6" | "slot7" | "slot8" => translate("Mochila"),
        "teleport0" => translate("Espaço do TP"),
        slot if slot.starts_with("stash") => translate("Stash"),
        // O GSI põe o aprimoramento do neutro (`item_enhancement_*`) no
        // segundo espaço neutro.
        "neutral1" => translate("Aprimoramento neutro"),
        slot if slot.starts_with("neutral") => translate("Item neutro"),
        other => other.to_owned(),
    }
}

/// Herói escolhido com os dados embutidos, quando o catálogo o conhece.
pub fn picked_hero<'a>(game: &GameSnapshot, catalog: &'a HeroCatalog) -> Option<&'a HeroInfo> {
    let HeroPick::Picked { name, id } = &game.hero else {
        return None;
    };
    catalog
        .by_name(name)
        .or_else(|| id.and_then(|id| catalog.by_id(id)))
}

fn hero_rows(game: Option<&GameSnapshot>, hero: Option<&HeroInfo>) -> Vec<RowContent> {
    let Some(HeroPick::Picked { name, .. }) = game.map(|game| &game.hero) else {
        return vec![RowContent::new(
            Level::Info,
            &translate("Nenhum herói escolhido ainda"),
            &translate("O herói aparece quando o Dota informa a escolha."),
        )];
    };
    let Some(hero) = hero else {
        return vec![RowContent::new(
            Level::Warning,
            &translate_with("Herói sem dados embutidos: {}", &[name]),
            &translate("Confirmado pelo GSI; os dados embutidos não têm este herói."),
        )];
    };
    let embedded_source = translate("Dados embutidos (dotaconstants)");
    let roles: Vec<String> = hero.roles.iter().map(role_label).collect();
    vec![
        RowContent::new(
            Level::Ok,
            &hero.display_name,
            &translate("Herói confirmado pelo GSI"),
        ),
        RowContent::new(
            Level::Info,
            &translate_with("Atributo: {}", &[&attribute_label(&hero.primary_attribute)]),
            &embedded_source,
        ),
        RowContent::new(
            Level::Info,
            &translate_with("Ataque: {}", &[&attack_label(&hero.attack_type)]),
            &embedded_source,
        ),
        RowContent::new(
            Level::Info,
            &translate_with("Funções: {}", &[&roles.join(", ")]),
            &embedded_source,
        ),
    ]
}

fn ability_rows(game: Option<&GameSnapshot>, hero: Option<&HeroInfo>) -> Vec<RowContent> {
    let Some((source, abilities)) = ability_lines(game, hero) else {
        return vec![RowContent::new(
            Level::Info,
            &translate("Sem habilidades para mostrar"),
            &translate("Escolha um herói ou aguarde o pré-jogo."),
        )];
    };
    let source_label = match source {
        AbilitySource::Gsi => translate("Nível confirmado pelo GSI"),
        AbilitySource::Embedded => {
            translate("Dados embutidos; o nível aparece quando o GSI enviar as habilidades")
        }
    };
    abilities
        .iter()
        .map(|ability| {
            let title = match (ability.level, ability.ultimate) {
                (Some(level), true) => translate_with(
                    "{} (ultimate) — nível {}",
                    &[&ability.name, &level.to_string()],
                ),
                (Some(level), false) => {
                    translate_with("{} — nível {}", &[&ability.name, &level.to_string()])
                }
                (None, _) => ability.name.clone(),
            };
            RowContent::new(Level::Info, &title, &source_label)
        })
        .collect()
}

/// Habilidades a exibir: as do GSI, sem as cosméticas, com nome de exibição
/// quando o catálogo o conhece; sem elas, a lista dos dados embutidos.
pub fn ability_lines(
    game: Option<&GameSnapshot>,
    hero: Option<&HeroInfo>,
) -> Option<(AbilitySource, Vec<AbilityLine>)> {
    if let Some(abilities) = game.and_then(|game| game.abilities.as_ref()) {
        let lines = abilities
            .iter()
            .filter(|ability| !ability.is_cosmetic())
            .map(|ability| AbilityLine {
                name: hero
                    .and_then(|hero| hero.ability_display_name(&ability.name))
                    .unwrap_or(&ability.name)
                    .to_owned(),
                level: Some(ability.level),
                ultimate: ability.ultimate,
            })
            .collect();
        return Some((AbilitySource::Gsi, lines));
    }
    let hero = hero?;
    let lines = hero
        .abilities
        .iter()
        .map(|ability| AbilityLine {
            name: ability.display_name.clone(),
            level: None,
            ultimate: false,
        })
        .collect();
    Some((AbilitySource::Embedded, lines))
}

/// Origem e horário de um evento, como aparece abaixo do texto dele.
fn timeline_subtitle(entry: &TimelineEntry) -> String {
    format!(
        "{} · {}",
        provenance_label(entry.event.provenance),
        format_time(entry.at)
    )
}

/// Texto de um evento da partida, com os nomes de exibição de herói,
/// habilidade e itens quando os catálogos os conhecem.
pub fn event_label(kind: &MatchEventKind, catalog: &HeroCatalog, items: &ItemCatalog) -> String {
    let item_name = |item: &str| items.display_name(item).to_owned();
    match kind {
        MatchEventKind::MatchFound => translate("Partida encontrada"),
        MatchEventKind::HeroPicked { hero } => {
            let name = catalog
                .by_name(hero)
                .map_or(hero.as_str(), |info| info.display_name.as_str());
            translate_with("Herói escolhido: {}", &[name])
        }
        MatchEventKind::MatchStarted => translate("Partida começou"),
        MatchEventKind::MatchEnded { result } => match result {
            MatchResult::Victory => translate("Partida terminou: vitória"),
            MatchResult::Defeat => translate("Partida terminou: derrota"),
            MatchResult::Unknown => translate("Partida terminou"),
        },
        MatchEventKind::ItemAdded {
            item,
            placement: ItemPlacement::Carried,
        } => translate_with("Item no inventário: {}", &[&item_name(item)]),
        MatchEventKind::ItemAdded {
            item,
            placement: ItemPlacement::Neutral,
        } => translate_with("Item neutro equipado: {}", &[&item_name(item)]),
        MatchEventKind::ItemRemoved {
            item,
            placement: ItemPlacement::Carried,
        } => translate_with("Item saiu do inventário: {}", &[&item_name(item)]),
        MatchEventKind::ItemRemoved {
            item,
            placement: ItemPlacement::Neutral,
        } => translate_with("Item neutro removido: {}", &[&item_name(item)]),
        MatchEventKind::ItemAssembled { item, components } => {
            let component_names: Vec<String> = components
                .iter()
                .map(|component| item_name(component))
                .collect();
            translate_with(
                "Item montado: {} ({})",
                &[&item_name(item), &component_names.join(", ")],
            )
        }
        MatchEventKind::AbilityLeveled { ability, level } => {
            let name = catalog
                .ability_display_name(ability)
                .unwrap_or(ability.as_str());
            translate_with("{} subiu para o nível {}", &[name, &level.to_string()])
        }
        MatchEventKind::HeroDied => translate("Herói morreu"),
    }
}

/// De onde vem a informação, como aparece ao lado de cada evento.
pub fn provenance_label(provenance: Provenance) -> String {
    match provenance {
        Provenance::Confirmed => translate("Confirmado pelo GSI"),
        Provenance::Inferred => translate("Inferido pelo aplicativo"),
        Provenance::Manual => translate("Marcado por você"),
    }
}

pub fn attribute_label(attribute: &PrimaryAttribute) -> String {
    match attribute {
        PrimaryAttribute::Strength => translate("Força"),
        PrimaryAttribute::Agility => translate("Agilidade"),
        PrimaryAttribute::Intelligence => translate("Inteligência"),
        PrimaryAttribute::Universal => translate("Universal"),
        PrimaryAttribute::Other(code) => code.clone(),
    }
}

pub fn attack_label(attack_type: &AttackType) -> String {
    match attack_type {
        AttackType::Melee => translate("Corpo a corpo"),
        AttackType::Ranged => translate("À distância"),
        AttackType::Other(name) => name.clone(),
    }
}

pub fn role_label(role: &HeroRole) -> String {
    match role {
        HeroRole::Carry => translate("Carry"),
        HeroRole::Support => translate("Suporte"),
        HeroRole::Nuker => translate("Dano explosivo"),
        HeroRole::Disabler => translate("Controle"),
        HeroRole::Durable => translate("Resistente"),
        HeroRole::Escape => translate("Fuga"),
        HeroRole::Pusher => translate("Pressão em torres"),
        HeroRole::Initiator => translate("Iniciador"),
        HeroRole::Other(name) => name.clone(),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/hero_tests.rs"]
mod tests;
