//! Tela "Herói": herói do jogador com os dados embutidos, as habilidades que
//! o GSI informa e a linha do tempo da partida. Cada informação diz de onde
//! vem: dado embutido, confirmado pelo GSI, inferido ou marcado pelo usuário.

use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};
use tokio::sync::mpsc::UnboundedSender;

use crate::application::error::{Redactor, ToDiagnostic, log_error};
use crate::application::event_bus::{CoreConnectionError, UiCommand};
use crate::application::state::{AppSnapshot, MatchStatus, TimelineEntry};
use crate::domain::game::{GameSnapshot, HeroPick};
use crate::domain::hero_catalog::{AttackType, HeroCatalog, HeroInfo, HeroRole, PrimaryAttribute};
use crate::domain::match_events::{MatchEventKind, MatchResult, Provenance};
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{
    Level, boxed_list, diagnostic_row, format_time, section_title, status_row,
};

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

/// Página Herói.
pub struct HeroPage {
    commands: UnboundedSender<UiCommand>,
    catalog: Arc<HeroCatalog>,
    match_status: MatchStatus,
    redactor: Redactor,
    /// Falha de comunicação com o núcleo, mostrada na própria página.
    core_failure: Option<CoreConnectionError>,
}

/// Mensagens recebidas pela página Herói.
#[derive(Debug)]
pub enum HeroMessage {
    Snapshot(Box<AppSnapshot>),
    CoreStopped,
    MarkMatchFound,
}

/// Dados para iniciar a página.
pub struct HeroPageInit {
    pub commands: UnboundedSender<UiCommand>,
    pub catalog: Arc<HeroCatalog>,
}

/// Widgets reconstruídos a cada novo estado.
pub struct HeroWidgets {
    hero_list: gtk::ListBox,
    ability_list: gtk::ListBox,
    timeline_list: gtk::ListBox,
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
            "Mostra o herói escolhido, suas habilidades e o que aconteceu na partida. \
             Funções vêm dos dados embutidos e não indicam a posição (1 a 5) na partida.",
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

        let hero_list = boxed_list();
        content.append(&section_title(&translate("Herói escolhido")));
        content.append(&hero_list);

        let ability_list = boxed_list();
        content.append(&section_title(&translate("Habilidades")));
        content.append(&ability_list);

        let timeline_list = boxed_list();
        content.append(&section_title(&translate("Linha do tempo da partida")));
        content.append(&timeline_list);

        let clamp = adw::Clamp::builder()
            .maximum_size(860)
            .child(&content)
            .build();
        root.set_child(Some(&clamp));

        let model = HeroPage {
            commands: init.commands,
            catalog: init.catalog,
            match_status: MatchStatus::default(),
            redactor: Redactor::default(),
            core_failure: None,
        };
        let widgets = HeroWidgets {
            hero_list,
            ability_list,
            timeline_list,
        };
        model.render(&widgets);
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            HeroMessage::Snapshot(snapshot) => {
                self.match_status = snapshot.match_status;
                self.redactor = snapshot.redactor;
            }
            HeroMessage::CoreStopped => self.report_core_failure(CoreConnectionError::StateClosed),
            HeroMessage::MarkMatchFound => {
                if let Err(core_error) = send_command(&self.commands, UiCommand::MarkMatchFound) {
                    self.report_core_failure(core_error);
                }
            }
        }
        self.render(widgets);
    }
}

impl HeroPage {
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

    fn render(&self, widgets: &HeroWidgets) {
        let game = self.match_status.game.as_ref();
        let hero = game.and_then(|game| picked_hero(game, &self.catalog));

        widgets.hero_list.remove_all();
        if let Some(core_failure) = &self.core_failure {
            widgets
                .hero_list
                .append(&diagnostic_row(&core_failure.to_diagnostic(&self.redactor)));
        }
        for row in hero_rows(game, hero) {
            widgets.hero_list.append(&row);
        }

        widgets.ability_list.remove_all();
        for row in ability_rows(game, hero) {
            widgets.ability_list.append(&row);
        }

        widgets.timeline_list.remove_all();
        if self.match_status.timeline.is_empty() {
            widgets.timeline_list.append(&status_row(
                Level::Info,
                &translate("Nenhum evento da partida ainda"),
                &translate("Os eventos aparecem quando o Dota envia dados de uma partida."),
            ));
        }
        for entry in self.match_status.timeline.iter().rev() {
            widgets
                .timeline_list
                .append(&timeline_row(entry, &self.catalog));
        }
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

fn hero_rows(game: Option<&GameSnapshot>, hero: Option<&HeroInfo>) -> Vec<adw::ActionRow> {
    let Some(HeroPick::Picked { name, .. }) = game.map(|game| &game.hero) else {
        return vec![status_row(
            Level::Info,
            &translate("Nenhum herói escolhido ainda"),
            &translate("O herói aparece quando o Dota informa a escolha."),
        )];
    };
    let Some(hero) = hero else {
        return vec![status_row(
            Level::Warning,
            &translate_with("Herói sem dados embutidos: {}", &[name]),
            &translate("Confirmado pelo GSI; os dados embutidos não têm este herói."),
        )];
    };
    let embedded_source = translate("Dados embutidos (dotaconstants)");
    let roles: Vec<String> = hero.roles.iter().map(role_label).collect();
    vec![
        status_row(
            Level::Ok,
            &hero.display_name,
            &translate("Herói confirmado pelo GSI"),
        ),
        status_row(
            Level::Info,
            &translate_with("Atributo: {}", &[&attribute_label(&hero.primary_attribute)]),
            &embedded_source,
        ),
        status_row(
            Level::Info,
            &translate_with("Ataque: {}", &[&attack_label(&hero.attack_type)]),
            &embedded_source,
        ),
        status_row(
            Level::Info,
            &translate_with("Funções: {}", &[&roles.join(", ")]),
            &embedded_source,
        ),
    ]
}

fn ability_rows(game: Option<&GameSnapshot>, hero: Option<&HeroInfo>) -> Vec<adw::ActionRow> {
    let Some((source, abilities)) = ability_lines(game, hero) else {
        return vec![status_row(
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
            status_row(Level::Info, &title, &source_label)
        })
        .collect()
}

/// Habilidades a exibir: as do GSI, com nome de exibição quando o catálogo
/// o conhece; sem elas, a lista dos dados embutidos.
pub fn ability_lines(
    game: Option<&GameSnapshot>,
    hero: Option<&HeroInfo>,
) -> Option<(AbilitySource, Vec<AbilityLine>)> {
    if let Some(abilities) = game.and_then(|game| game.abilities.as_ref()) {
        let lines = abilities
            .iter()
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

fn timeline_row(entry: &TimelineEntry, catalog: &HeroCatalog) -> adw::ActionRow {
    status_row(
        Level::Info,
        &event_label(&entry.event.kind, catalog),
        &format!(
            "{} · {}",
            provenance_label(entry.event.provenance),
            format_time(entry.at)
        ),
    )
}

/// Texto de um evento da partida.
pub fn event_label(kind: &MatchEventKind, catalog: &HeroCatalog) -> String {
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
        MatchEventKind::ItemAdded { item } => translate_with("Item no inventário: {}", &[item]),
        MatchEventKind::ItemRemoved { item } => {
            translate_with("Item saiu do inventário: {}", &[item])
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
