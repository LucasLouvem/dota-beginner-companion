//! Tela "Itens": builds de partidas profissionais e públicas para o herói e
//! o modo da partida, e a popularidade de itens por fase. Cada bloco diz a
//! fonte, o patch, se os dados são antigos e o tamanho da amostra.

use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::state::{AppSnapshot, RecommendationStatus};
use crate::application::use_cases::fallback::DataOrigin;
use crate::application::use_cases::recommendations::Recommendations;
use crate::domain::builds::{BuildEvidence, MIN_SAMPLE, RankedBuild};
use crate::domain::game::GameMode;
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::PopularityPhase;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{
    Level, RowContent, RowSection, boxed_list, format_duration, format_time, section_title,
};

use super::hero::mode_label;

/// Itens mostrados por fase na popularidade.
const POPULAR_ITEMS_SHOWN: usize = 5;

/// Conteúdo das seções da aba, na ordem exibida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemsContent {
    pub header: Vec<RowContent>,
    pub notes: Vec<RowContent>,
    pub pro: Vec<RowContent>,
    pub public: Vec<RowContent>,
    pub popularity: Vec<RowContent>,
}

/// Página Itens.
pub struct ItemsPage {
    heroes: Arc<HeroCatalog>,
    items: Arc<ItemCatalog>,
    recommendations: RecommendationStatus,
    redactor: Redactor,
}

/// Mensagens recebidas pela página Itens.
#[derive(Debug)]
pub enum ItemsMessage {
    Snapshot(Box<AppSnapshot>),
}

/// Dados para iniciar a página.
pub struct ItemsPageInit {
    pub heroes: Arc<HeroCatalog>,
    pub items: Arc<ItemCatalog>,
}

/// Widgets da página, com o que cada lista mostra agora.
pub struct ItemsWidgets {
    header: RowSection,
    notes: RowSection,
    pro: RowSection,
    public: RowSection,
    popularity: RowSection,
}

impl Component for ItemsPage {
    type Init = ItemsPageInit;
    type Input = ItemsMessage;
    type Output = ();
    type CommandOutput = ();
    type Root = gtk::ScrolledWindow;
    type Widgets = ItemsWidgets;

    fn init_root() -> Self::Root {
        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build()
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(16);
        content.set_margin_end(16);

        let title = gtk::Label::new(Some(&translate("Itens")));
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let intro = gtk::Label::new(Some(&translate(
            "Builds observadas em partidas para o herói e o modo da partida, com dados do \
             OpenDota. São sugestões estatísticas: cada build mostra a taxa de vitória \
             observada e quantas partidas foram analisadas.",
        )));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");
        content.append(&title);
        content.append(&intro);

        let section = |text: Option<&str>| {
            if let Some(text) = text {
                content.append(&section_title(&translate(text)));
            }
            let list = boxed_list();
            content.append(&list);
            RowSection::new(list)
        };
        let header = section(None);
        let notes = section(None);
        let pro = section(Some("Builds de partidas profissionais"));
        let public = section(Some("Builds de partidas públicas"));
        let popularity = section(Some("Itens mais comprados por fase (profissionais)"));

        let clamp = adw::Clamp::builder()
            .maximum_size(860)
            .child(&content)
            .build();
        root.set_child(Some(&clamp));

        let model = ItemsPage {
            heroes: init.heroes,
            items: init.items,
            recommendations: RecommendationStatus::default(),
            redactor: Redactor::default(),
        };
        let mut widgets = ItemsWidgets {
            header,
            notes,
            pro,
            public,
            popularity,
        };
        model.render(&mut widgets);
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
            ItemsMessage::Snapshot(snapshot) => {
                self.recommendations = snapshot.recommendations;
                self.redactor = snapshot.redactor;
            }
        }
        self.render(widgets);
    }
}

impl ItemsPage {
    /// Atualiza só as linhas que mudaram desde o último estado exibido.
    fn render(&self, widgets: &mut ItemsWidgets) {
        let content = items_content(
            &self.recommendations,
            &self.heroes,
            &self.items,
            &self.redactor,
        );
        widgets.header.show(content.header);
        widgets.notes.show(content.notes);
        widgets.pro.show(content.pro);
        widgets.public.show(content.public);
        widgets.popularity.show(content.popularity);
    }
}

/// Conteúdo da aba para o estado das recomendações.
pub fn items_content(
    status: &RecommendationStatus,
    heroes: &HeroCatalog,
    items: &ItemCatalog,
    redactor: &Redactor,
) -> ItemsContent {
    let hero_name = |hero_id: u32| {
        heroes.by_id(hero_id).map_or_else(
            || translate_with("herói {}", &[&hero_id.to_string()]),
            |hero| hero.display_name.clone(),
        )
    };
    match status {
        RecommendationStatus::Unavailable => ItemsContent {
            header: vec![RowContent::new(
                Level::Warning,
                &translate("Recomendações indisponíveis"),
                &translate(
                    "O banco local ou o cliente HTTP não puderam ser abertos; os detalhes estão no log.",
                ),
            )],
            ..ItemsContent::default()
        },
        RecommendationStatus::Idle => ItemsContent {
            header: vec![RowContent::new(
                Level::Info,
                &translate("Nenhum herói para recomendar"),
                &translate("As recomendações aparecem quando o Dota informa o herói escolhido."),
            )],
            ..ItemsContent::default()
        },
        RecommendationStatus::Loading { key } => ItemsContent {
            header: vec![RowContent::new(
                Level::Info,
                &translate_with("Carregando recomendações de {}", &[&hero_name(key.hero_id)]),
                &mode_label(key.mode),
            )],
            ..ItemsContent::default()
        },
        RecommendationStatus::Interrupted { interrupted, .. } => ItemsContent {
            header: vec![RowContent::diagnostic(&interrupted.to_diagnostic(redactor))],
            ..ItemsContent::default()
        },
        RecommendationStatus::Ready(recommendations) => ready_content(
            recommendations,
            &hero_name(recommendations.key.hero_id),
            items,
            redactor,
        ),
    }
}

fn ready_content(
    recommendations: &Recommendations,
    hero_name: &str,
    items: &ItemCatalog,
    redactor: &Redactor,
) -> ItemsContent {
    let mode = recommendations.key.mode;
    let patch = &recommendations.patch.name;
    let header = vec![RowContent::new(
        Level::Info,
        &translate_with("{} · {} · patch {}", &[hero_name, &mode_label(mode), patch]),
        &translate_with(
            "Consultado às {}",
            &[&format_time(recommendations.loaded_at)],
        ),
    )];
    let mut notes = vec![RowContent::new(
        Level::Info,
        &translate(
            "Dados públicos de todas as faixas de rank; a recomendação não considera posição.",
        ),
        &translate("A posição (1 a 5) não vem do Dota, e as builds misturam todas as posições."),
    )];
    if mode == GameMode::Bots {
        notes.push(RowContent::new(
            Level::Info,
            &translate(
                "Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores.",
            ),
            &translate("Não há dados de partidas contra bots nas recomendações."),
        ));
    }

    let mut pro = Vec::new();
    if mode == GameMode::Turbo {
        pro.push(RowContent::new(
            Level::Warning,
            &translate("Partidas profissionais não são turbo"),
            &translate("Os tempos de compra destas builds não valem para o ritmo do turbo."),
        ));
    }
    let pro_builds = &recommendations.pro;
    pro.extend(
        pro_builds
            .ranking
            .builds
            .iter()
            .map(|build| build_row(build, items)),
    );
    if pro_builds.ranking.builds.is_empty() {
        pro.push(insufficient_sample(
            pro_builds.valid_matches,
            &translate("partidas profissionais válidas"),
        ));
    }
    pro.push(source_row(
        &translate_with(
            "Fonte: OpenDota, partidas profissionais de liga · patch {}",
            &[patch],
        ),
        pro_builds.candidates_origin,
    ));
    if let Some(error) = &pro_builds.error {
        pro.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }

    let mut public: Vec<RowContent> = recommendations
        .public
        .builds
        .iter()
        .map(|build| build_row(build, items))
        .collect();
    if recommendations.public.builds.is_empty() {
        public.push(insufficient_sample(
            recommendations.public.analyzed,
            &translate("partidas públicas coletadas deste modo"),
        ));
    }
    public.push(RowContent::new(
        Level::Info,
        &translate_with(
            "Fonte: partidas públicas coletadas do OpenDota · patch {}",
            &[patch],
        ),
        &translate("A coleta continua em segundo plano enquanto o aplicativo está aberto."),
    ));
    if let Some(error) = &recommendations.public_error {
        public.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }

    ItemsContent {
        header,
        notes,
        pro,
        public,
        popularity: popularity_rows(recommendations, items, redactor),
    }
}

/// Linha de uma build: itens na ordem de compra (ou sem ordem), taxa de
/// vitória observada, amostra e tempos médios quando há ordem.
pub fn build_row(build: &RankedBuild, items: &ItemCatalog) -> RowContent {
    let names: Vec<&str> = build
        .key
        .items
        .iter()
        .map(|item| items.display_name(item))
        .collect();
    let (title, detail) = match build.key.evidence {
        BuildEvidence::OrderedPurchaseLog => (
            names.join(" → "),
            build
                .average_times
                .as_ref()
                .map(|times| {
                    let times: Vec<String> =
                        times.iter().map(|time| purchase_time(*time)).collect();
                    translate_with("tempo médio: {}", &[&times.join(", ")])
                })
                .unwrap_or_default(),
        ),
        BuildEvidence::FinalInventoryUnordered => (
            names.join(" + "),
            translate("itens finais, sem ordem de compra"),
        ),
    };
    let rate = translate_with(
        "Taxa de vitória observada: {}% em {} partidas",
        &[
            &win_percent(build.wins, build.matches).to_string(),
            &build.matches.to_string(),
        ],
    );
    let subtitle = if detail.is_empty() {
        rate
    } else {
        format!("{rate} · {detail}")
    };
    RowContent::new(Level::Info, &title, &subtitle)
}

/// Porcentagem arredondada de `wins` em `matches`, sem ponto flutuante.
pub fn win_percent(wins: usize, matches: usize) -> usize {
    if matches == 0 {
        return 0;
    }
    (wins * 200 + matches) / (2 * matches)
}

/// Momento de compra no relógio da partida; antes do início, sem relógio.
pub fn purchase_time(seconds: i32) -> String {
    match u32::try_from(seconds) {
        Ok(seconds) => format_duration(seconds),
        Err(_) => translate("antes do início"),
    }
}

fn insufficient_sample(analyzed: usize, matches_kind: &str) -> RowContent {
    RowContent::new(
        Level::Info,
        &translate("Amostra insuficiente"),
        &translate_with(
            "Nenhuma build com pelo menos {} partidas entre {} {}.",
            &[&MIN_SAMPLE.to_string(), &analyzed.to_string(), matches_kind],
        ),
    )
}

fn source_row(source: &str, origin: Option<DataOrigin>) -> RowContent {
    let (level, freshness) = match origin {
        Some(DataOrigin::Live | DataOrigin::Cached) => (Level::Info, translate("Dados atuais")),
        Some(DataOrigin::Stale) => (
            Level::Warning,
            translate(
                "Dados antigos: o OpenDota não respondeu, então aparece o que estava guardado.",
            ),
        ),
        Some(DataOrigin::Embedded) | None => (Level::Warning, translate("Sem dados")),
    };
    RowContent::new(level, source, &freshness)
}

fn popularity_rows(
    recommendations: &Recommendations,
    items: &ItemCatalog,
    redactor: &Redactor,
) -> Vec<RowContent> {
    let popularity = &recommendations.popularity;
    let mut rows: Vec<RowContent> = popularity
        .value
        .iter()
        .flat_map(|value| &value.phases)
        .filter(|(_, counts)| !counts.is_empty())
        .map(|(phase, counts)| {
            let names: Vec<&str> = counts
                .iter()
                .take(POPULAR_ITEMS_SHOWN)
                .map(|(item, _)| items.display_name(item))
                .collect();
            RowContent::new(
                Level::Info,
                &translate_with("{}: {}", &[&phase_label(*phase), &names.join(", ")]),
                &translate("Popularidade em partidas profissionais, não taxa de vitória."),
            )
        })
        .collect();
    rows.push(source_row(
        &translate_with(
            "Fonte: OpenDota, popularidade em partidas profissionais · patch {}",
            &[&recommendations.patch.name],
        ),
        popularity.origin,
    ));
    if popularity.value.is_none()
        && let Some(error) = &popularity.error
    {
        rows.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    rows
}

pub fn phase_label(phase: PopularityPhase) -> String {
    match phase {
        PopularityPhase::Start => translate("Início"),
        PopularityPhase::Early => translate("Até 10 minutos"),
        PopularityPhase::Mid => translate("De 10 a 25 minutos"),
        PopularityPhase::Late => translate("Depois de 25 minutos"),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/items_tests.rs"]
mod tests;
