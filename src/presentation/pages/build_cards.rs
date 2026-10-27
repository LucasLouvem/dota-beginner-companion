//! Conteúdo dos cards da página Herói que não dependem da build escolhida:
//! o cabeçalho do herói. Sem widgets, para poder ser testado.

use crate::application::state::{MatchStatus, RecommendationStatus};
use crate::domain::game::GamePhase;
use crate::domain::hero_catalog::{HeroCatalog, HeroInfo};
use crate::presentation::i18n::{translate, translate_with};

use super::build::hero_name;
use super::hero::{attribute_label, last_match_snapshot, picked_hero, result_lines, role_label};

/// Cabeçalho do herói: nome, atributo e funções (dados fixos do herói) e a
/// origem dos dados. O modo de jogo é perguntado numa janela quando a partida
/// é encontrada e não aparece na página.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeroHeader {
    pub name: String,
    /// Atributo principal e funções, um rótulo cada.
    pub chips: Vec<String>,
    /// Patch e fonte das recomendações.
    pub source: String,
    /// Resultado da última partida, enquanto ela é mostrada.
    pub last_match: String,
    /// Nome interno do herói, que identifica o retrato.
    pub image: Option<String>,
    /// O jogador está numa partida (da espera dos jogadores ao jogo em
    /// andamento).
    pub in_match: bool,
}

/// Há uma partida em andamento no GSI: da espera dos jogadores até o jogo,
/// sem o pós-jogo, o menu e fases que o aplicativo não trata.
pub fn in_match(status: &MatchStatus) -> bool {
    status.game.as_ref().is_some_and(|game| {
        matches!(
            game.phase,
            Some(
                GamePhase::WaitForPlayersToLoad
                    | GamePhase::HeroSelection
                    | GamePhase::StrategyTime
                    | GamePhase::TeamShowcase
                    | GamePhase::WaitForMapToLoad
                    | GamePhase::PreGame
                    | GamePhase::InProgress
            )
        )
    })
}

/// Herói do cabeçalho: o da partida atual; sem ele, o da última partida; sem
/// ele, o da consulta das recomendações (a build escolhida é dele).
pub fn header_hero<'a>(
    status: &MatchStatus,
    recommendations: &RecommendationStatus,
    catalog: &'a HeroCatalog,
) -> Option<&'a HeroInfo> {
    status
        .game
        .as_ref()
        .and_then(|game| picked_hero(game, catalog))
        .or_else(|| {
            status
                .last_match
                .as_ref()
                .and_then(|summary| picked_hero(&last_match_snapshot(summary), catalog))
        })
        .or_else(|| match recommendations {
            RecommendationStatus::Ready(ready) => catalog.by_id(ready.key.hero_id),
            _ => None,
        })
}

pub fn hero_header(
    status: &MatchStatus,
    recommendations: &RecommendationStatus,
    catalog: &HeroCatalog,
) -> HeroHeader {
    let hero = header_hero(status, recommendations, catalog);
    let name = match (hero, recommendations) {
        (Some(hero), _) => hero.display_name.clone(),
        (None, RecommendationStatus::Ready(ready)) => hero_name(catalog, ready.key.hero_id),
        (None, _) => translate("Nenhum herói escolhido"),
    };
    let chips = hero
        .map(|hero| {
            std::iter::once(attribute_label(&hero.primary_attribute))
                .chain(hero.roles.iter().map(role_label))
                .collect()
        })
        .unwrap_or_default();
    let source = match recommendations {
        RecommendationStatus::Ready(ready) => {
            translate_with("Patch {} · Fonte: OpenDota", &[&ready.patch.name])
        }
        _ => String::new(),
    };
    // A última partida só é mostrada enquanto a atual não tem herói.
    let current_has_hero = status
        .game
        .as_ref()
        .is_some_and(|game| picked_hero(game, catalog).is_some());
    let last_match = match &status.last_match {
        Some(summary) if !current_has_hero => {
            format!(
                "{} · {}",
                translate("Última partida"),
                result_lines(summary).join(" · ")
            )
        }
        _ => String::new(),
    };
    HeroHeader {
        name,
        chips,
        source,
        last_match,
        image: hero.map(|hero| hero.name.clone()),
        in_match: in_match(status),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/build_cards_tests.rs"]
mod tests;
