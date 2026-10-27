//! Texto e regras de exibição da linha do tempo de uma partida: a descrição
//! de cada evento, a origem (confirmado, inferido ou manual) e a comparação
//! entre a linha exibida e a nova. A aba Herói deixou de mostrá-la; ela serve
//! à aba Histórico (#39 e #41).

use crate::application::state::TimelineEntry;
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_events::{ItemPlacement, MatchEventKind, MatchResult, Provenance};
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::format_time;

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

/// Origem e horário de um evento, como aparece abaixo do texto dele.
pub fn timeline_subtitle(entry: &TimelineEntry) -> String {
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

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/timeline_tests.rs"]
mod tests;
