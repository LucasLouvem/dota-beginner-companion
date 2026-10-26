//! Página da aba Dados profissionais, que mostra as partidas profissionais
//! recentes do herói. A função pura de conteúdo define as seções; a página só
//! monta as listas e atualiza as linhas que mudaram. Itens e habilidades
//! ficam na página da build, dentro da aba Herói.

use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, adw, gtk};

use crate::application::error::Redactor;
use crate::application::state::{AppSnapshot, RecommendationStatus};
use crate::domain::hero_catalog::HeroCatalog;
use crate::presentation::i18n::translate;
use crate::presentation::widgets::{RowContent, RowSection, boxed_list, section_title};

use super::pro::pro_content;

/// Qual aba de recomendação a página mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationTab {
    ProMatches,
}

impl RecommendationTab {
    /// Título da aba, na barra lateral e no topo da página.
    pub fn title(self) -> String {
        match self {
            RecommendationTab::ProMatches => translate("Dados profissionais"),
        }
    }

    fn intro(self) -> String {
        match self {
            RecommendationTab::ProMatches => {
                translate("Partidas profissionais recentes com o herói, com dados do OpenDota.")
            }
        }
    }

    /// Título de cada seção, na ordem do conteúdo; `None` é seção sem título.
    fn section_titles(self) -> Vec<Option<String>> {
        match self {
            RecommendationTab::ProMatches => {
                vec![None, Some(translate("Partidas profissionais recentes"))]
            }
        }
    }
}

/// Catálogos embutidos usados pelos nomes.
#[derive(Clone)]
pub struct RecommendationCatalogs {
    pub heroes: Arc<HeroCatalog>,
}

/// Conteúdo de cada seção da aba para o estado das recomendações.
pub fn tab_content(
    tab: RecommendationTab,
    status: &RecommendationStatus,
    catalogs: &RecommendationCatalogs,
    redactor: &Redactor,
) -> Vec<Vec<RowContent>> {
    match tab {
        RecommendationTab::ProMatches => pro_content(status, &catalogs.heroes, redactor),
    }
}

/// Página de uma aba de recomendação.
pub struct RecommendationPage {
    tab: RecommendationTab,
    catalogs: RecommendationCatalogs,
    recommendations: RecommendationStatus,
    redactor: Redactor,
}

/// Mensagens recebidas pela página.
#[derive(Debug)]
pub enum RecommendationMessage {
    Snapshot(Box<AppSnapshot>),
}

/// Dados para iniciar a página.
pub struct RecommendationPageInit {
    pub tab: RecommendationTab,
    pub catalogs: RecommendationCatalogs,
}

/// Uma lista por seção, na ordem do conteúdo.
pub struct RecommendationWidgets {
    sections: Vec<RowSection>,
}

impl Component for RecommendationPage {
    type Init = RecommendationPageInit;
    type Input = RecommendationMessage;
    type Output = ();
    type CommandOutput = ();
    type Root = gtk::ScrolledWindow;
    type Widgets = RecommendationWidgets;

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

        let title = gtk::Label::new(Some(&init.tab.title()));
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let intro = gtk::Label::new(Some(&init.tab.intro()));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");
        content.append(&title);
        content.append(&intro);

        let sections = init
            .tab
            .section_titles()
            .into_iter()
            .map(|section| {
                if let Some(section) = section {
                    content.append(&section_title(&section));
                }
                let list = boxed_list();
                content.append(&list);
                RowSection::new(list)
            })
            .collect();

        let clamp = adw::Clamp::builder()
            .maximum_size(860)
            .child(&content)
            .build();
        root.set_child(Some(&clamp));

        let model = RecommendationPage {
            tab: init.tab,
            catalogs: init.catalogs,
            recommendations: RecommendationStatus::default(),
            redactor: Redactor::default(),
        };
        let mut widgets = RecommendationWidgets { sections };
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
            RecommendationMessage::Snapshot(snapshot) => {
                self.recommendations = snapshot.recommendations;
                self.redactor = snapshot.redactor;
            }
        }
        self.render(widgets);
    }
}

impl RecommendationPage {
    /// Atualiza só as linhas que mudaram desde o último estado exibido.
    fn render(&self, widgets: &mut RecommendationWidgets) {
        let content = tab_content(
            self.tab,
            &self.recommendations,
            &self.catalogs,
            &self.redactor,
        );
        for (section, rows) in widgets.sections.iter_mut().zip(content) {
            section.show(rows);
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/recommendation_page_tests.rs"]
mod tests;
