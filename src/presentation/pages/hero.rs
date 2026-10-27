//! Tela "Herói": o componente que liga o estado do aplicativo à página da
//! build (vitórias recentes para escolher, com os itens e a ordem de
//! habilidades acompanhados pela partida, e a comparação com a build). Guarda
//! a build escolhida, só da interface, e, quando uma partida é encontrada,
//! pergunta o modo de jogo numa janela sobre o aplicativo.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};
use tokio::sync::mpsc::UnboundedSender;

use crate::application::error::{Redactor, ToDiagnostic, log_error};
use crate::application::event_bus::{CoreConnectionError, UiCommand};
use crate::application::state::{
    AppSnapshot, MatchStatus, ModeOrigin, ModeState, RecommendationStatus,
};
use crate::application::use_cases::item_icons::ItemIcons;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::application::use_cases::recommendations::{BuildId, Recommendations};
use crate::domain::ability_catalog::AbilityIdCatalog;
use crate::domain::game::{GameMode, GamePhase, GameSnapshot, HeroPick};
use crate::domain::hero_catalog::{HeroCatalog, HeroInfo, HeroRole, PrimaryAttribute};
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_events::{MatchResult, MatchSummary};
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{RowContent, format_duration};

use super::build::{
    BuildCatalogs, SidebarItem, build_detail, entry_at, header_content, index_of, select_build,
    sidebar, sidebar_items,
};
use super::build_cards::hero_header;
use super::build_panel::{BuildPanel, BuildView};
use super::send_command;

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

/// Widgets da página: a página da build, que também mostra o cabeçalho do
/// herói, as estatísticas e a linha do tempo, e o botão do modo.
pub struct HeroWidgets {
    build: BuildPanel,
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
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);

        let match_found_button = gtk::Button::with_label(&translate("Partida encontrada"));
        match_found_button.set_tooltip_text(Some(&translate(
            "Marca manualmente que uma partida foi encontrada. O GSI não informa esse momento; \
             o aplicativo também o deduz quando a seleção de heróis começa.",
        )));
        {
            let button_sender = sender.clone();
            match_found_button
                .connect_clicked(move |_| button_sender.input(HeroMessage::MarkMatchFound));
        }
        let build = {
            let select_sender = sender.clone();
            BuildPanel::new(
                move |index| select_sender.input(HeroMessage::SelectBuildRow(index)),
                &match_found_button,
            )
        };
        content.append(build.widget());

        // Sem `Clamp`: a página usa a largura da janela, e a página da build
        // reorganiza os cards conforme a largura.
        // O `BreakpointBin` informa altura mínima zero; com a política
        // padrão do `Viewport` (mínima), a página nunca passaria da altura da
        // janela e cortaria o conteúdo. A política natural a faz rolar.
        let viewport = gtk::Viewport::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        viewport.set_vscroll_policy(gtk::ScrollablePolicy::Natural);
        viewport.set_child(Some(&content));
        root.set_child(Some(&viewport));

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
        let mut widgets = HeroWidgets { build };
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
                    let dialog = present_mode_dialog(root, match_number, preselected, sender);
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
                    self.sidebar = sidebar_items(&sidebar(&status));
                    self.sidebar_source = Some(Arc::clone(recommendations));
                }
                self.selection =
                    select_build(self.selection.as_ref(), &recommendations.key, &self.sidebar)
                        .map(|id| (recommendations.key.clone(), id));
            }
            _ => {
                self.sidebar = sidebar_items(&sidebar(&status));
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

    /// Atualiza só o que mudou desde o último estado exibido. O cabeçalho e
    /// a lista de builds não dependem da build escolhida; a build é
    /// acompanhada pelo estado atual da partida.
    fn render(&self, widgets: &mut HeroWidgets) {
        let game = self.match_status.game.as_ref();
        let catalogs = BuildCatalogs {
            heroes: &self.catalog,
            items: &self.items,
            abilities: &self.abilities,
        };
        let detail = match &self.recommendations {
            RecommendationStatus::Ready(recommendations) => self
                .selection
                .as_ref()
                .and_then(|(_, id)| build_detail(recommendations, id, game, &catalogs)),
            _ => None,
        };
        let selected = self
            .selection
            .as_ref()
            .and_then(|(_, id)| index_of(&self.sidebar, id));

        let mut notices: Vec<RowContent> = self
            .core_failure
            .as_ref()
            .map(|core_error| RowContent::diagnostic(&core_error.to_diagnostic(&self.redactor)))
            .into_iter()
            .collect();
        notices
            .extend(header_content(&self.recommendations, &self.catalog, &self.redactor).notices);
        notices.extend(detail.iter().flat_map(|detail| detail.notes.clone()));

        widgets.build.show(&BuildView {
            hero: hero_header(&self.match_status, &self.recommendations, &self.catalog),
            notices,
            sidebar: &self.sidebar,
            selected,
            detail: detail.as_ref(),
            icons: &self.item_icons,
        });
    }
}

/// Abre a pergunta do modo sobre a janela do aplicativo, com `preselected`
/// marcado. O sinal `response` é emitido uma vez ao fechar, pelo botão
/// "Confirmar" ou por Esc e pelo "X" (`close-response`); a trava garante uma
/// única resposta mesmo se o fechamento for forçado.
fn present_mode_dialog(
    root: &gtk::ScrolledWindow,
    match_number: u64,
    preselected: GameMode,
    sender: ComponentSender<HeroPage>,
) -> adw::AlertDialog {
    let body = translate(
        "O Dota não informa o modo da partida. Fechar sem confirmar mantém o modo marcado \
         ao abrir.",
    );
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
        let (mode, origin) = mode_dialog_outcome(confirmed, preselected, selected);
        sender.input(HeroMessage::ChooseMode {
            match_number,
            mode,
            origin,
        });
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
/// explícita; fechar a pergunta vale o modo marcado ao abrir, descartando
/// mudanças não confirmadas.
pub fn mode_dialog_outcome(
    confirmed: bool,
    preselected: GameMode,
    selected: GameMode,
) -> (GameMode, ModeOrigin) {
    if confirmed {
        (selected, ModeOrigin::Chosen)
    } else {
        (preselected, ModeOrigin::Default)
    }
}

pub fn mode_label(mode: GameMode) -> String {
    match mode {
        GameMode::AllPick => translate("Escolha Livre"),
        GameMode::Turbo => translate("Turbo"),
        GameMode::Bots => translate("Contra bots"),
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

/// Herói escolhido com os dados embutidos, quando o catálogo o conhece.
pub fn picked_hero<'a>(game: &GameSnapshot, catalog: &'a HeroCatalog) -> Option<&'a HeroInfo> {
    let HeroPick::Picked { name, id } = &game.hero else {
        return None;
    };
    catalog
        .by_name(name)
        .or_else(|| id.and_then(|id| catalog.by_id(id)))
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
