//! Eventos da partida deduzidos da sequência de estados do GSI. Cada evento
//! informa se foi confirmado pelo GSI, inferido pelo aplicativo ou marcado
//! pelo usuário, para que uma inferência nunca apareça como fato.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::game::{
    AbilityState, CarriedItem, GamePhase, GameSnapshot, HeroPick, MatchId, PlayerStats,
};
use super::item_catalog::ItemCatalog;

/// Espaço do Town Portal. O TP é usado e reposto o tempo todo, então mudanças
/// nele não entram na linha do tempo.
const TELEPORT_SLOT: &str = "teleport0";
/// Prefixo dos espaços de item neutro (`neutral0`, `neutral1`).
const NEUTRAL_SLOT_PREFIX: &str = "neutral";

/// De onde vem a informação de um evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// O GSI informa o fato diretamente.
    Confirmed,
    /// Deduzido pelo aplicativo a partir de mudanças no estado.
    Inferred,
    /// Marcado pelo usuário.
    Manual,
}

/// Resultado da partida para o jogador local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchResult {
    Victory,
    Defeat,
    /// O Dota não informou o vencedor ou o time do jogador.
    Unknown,
}

/// Onde o item está: entre os itens comuns (inventário, mochila, stash) ou no
/// espaço de item neutro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemPlacement {
    Carried,
    Neutral,
}

impl ItemPlacement {
    fn of_slot(slot: &str) -> Self {
        if slot.starts_with(NEUTRAL_SLOT_PREFIX) {
            ItemPlacement::Neutral
        } else {
            ItemPlacement::Carried
        }
    }
}

/// O que aconteceu na partida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchEventKind {
    /// A entrada na seleção de heróis, vinda do menu, indica uma partida nova.
    MatchFound,
    HeroPicked {
        hero: String,
    },
    MatchStarted,
    MatchEnded {
        result: MatchResult,
    },
    /// Um item a mais entre os carregados. Não diz se foi compra, troca ou drop.
    ItemAdded {
        item: String,
        placement: ItemPlacement,
    },
    /// Um item a menos entre os carregados: pode ter sido usado, vendido ou
    /// combinado num item cuja receita o catálogo não conhece.
    ItemRemoved {
        item: String,
        placement: ItemPlacement,
    },
    /// Um item entrou na mesma mudança em que saíram componentes da receita
    /// dele. `components` lista só os componentes que saíram.
    ItemAssembled {
        item: String,
        components: Vec<String>,
    },
    AbilityLeveled {
        ability: String,
        level: u32,
    },
    HeroDied,
}

/// Evento com a origem da informação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchEvent {
    pub kind: MatchEventKind,
    pub provenance: Provenance,
}

impl MatchEvent {
    fn confirmed(kind: MatchEventKind) -> Self {
        Self {
            kind,
            provenance: Provenance::Confirmed,
        }
    }

    fn inferred(kind: MatchEventKind) -> Self {
        Self {
            kind,
            provenance: Provenance::Inferred,
        }
    }

    /// "Partida encontrada" marcada pelo usuário.
    pub fn manual_match_found() -> Self {
        Self {
            kind: MatchEventKind::MatchFound,
            provenance: Provenance::Manual,
        }
    }
}

/// Acompanha os estados na ordem de chegada (nunca pelo `clock_time`, que
/// volta entre fases) e devolve os eventos de cada novo estado. Guarda o que
/// já aconteceu na partida para não repetir eventos quando a fase volta.
#[derive(Debug, Default)]
pub struct MatchTracker {
    /// Receitas usadas para reconhecer itens montados; vazio, nenhuma montagem
    /// é reconhecida.
    items: Arc<ItemCatalog>,
    previous: Option<GameSnapshot>,
    started: bool,
    ended: bool,
}

impl MatchTracker {
    pub fn new(items: Arc<ItemCatalog>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// Eventos causados por `current`, comparado ao estado anterior.
    pub fn observe(&mut self, current: GameSnapshot) -> Vec<MatchEvent> {
        let mut events = Vec::new();
        if self.is_new_match(&current) {
            self.started = false;
            self.ended = false;
            events.push(MatchEvent::inferred(MatchEventKind::MatchFound));
        }
        if self.is_match_start(&current) {
            self.started = true;
            events.push(MatchEvent::confirmed(MatchEventKind::MatchStarted));
        }
        if let Some(previous) = &self.previous {
            push_hero_picked(&mut events, previous, &current);
            push_ability_levels(&mut events, previous, &current);
            push_inventory_changes(&mut events, previous, &current, &self.items);
            if previous.hero_alive == Some(true) && current.hero_alive == Some(false) {
                events.push(MatchEvent::confirmed(MatchEventKind::HeroDied));
            }
        }
        if !self.ended && current.phase == Some(GamePhase::PostGame) {
            self.ended = true;
            events.push(MatchEvent::confirmed(MatchEventKind::MatchEnded {
                result: match_result(&current),
            }));
        }
        self.previous = Some(current);
        events
    }

    /// Uma partida nova começa na seleção de heróis vinda do menu, do fim de
    /// outra partida ou do primeiro evento recebido. Voltar à seleção a partir
    /// de uma fase posterior da mesma partida não conta.
    fn is_new_match(&self, current: &GameSnapshot) -> bool {
        if current.phase != Some(GamePhase::HeroSelection) {
            return false;
        }
        match self.previous.as_ref().map(|previous| &previous.phase) {
            None | Some(None) | Some(Some(GamePhase::PostGame)) => true,
            Some(Some(_)) => false,
        }
    }

    /// O início só é anunciado quando a partida foi acompanhada antes dele;
    /// entrar no aplicativo com a partida já em andamento não é um início.
    fn is_match_start(&self, current: &GameSnapshot) -> bool {
        if self.started || current.phase != Some(GamePhase::InProgress) {
            return false;
        }
        self.previous
            .as_ref()
            .and_then(|previous| previous.phase.as_ref())
            .and_then(GamePhase::sequence_position)
            .is_some_and(|position| Some(position) < GamePhase::InProgress.sequence_position())
    }
}

fn push_hero_picked(events: &mut Vec<MatchEvent>, previous: &GameSnapshot, current: &GameSnapshot) {
    if let (HeroPick::NotPicked, HeroPick::Picked { name, .. }) = (&previous.hero, &current.hero) {
        events.push(MatchEvent::confirmed(MatchEventKind::HeroPicked {
            hero: name.clone(),
        }));
    }
}

/// Só compara quando os dois estados trazem as habilidades do mesmo herói.
fn push_ability_levels(
    events: &mut Vec<MatchEvent>,
    previous: &GameSnapshot,
    current: &GameSnapshot,
) {
    let (Some(previous_abilities), Some(current_abilities)) =
        (&previous.abilities, &current.abilities)
    else {
        return;
    };
    if previous.hero != current.hero {
        return;
    }
    for ability in current_abilities {
        let leveled = previous_abilities
            .iter()
            .find(|previous_ability| previous_ability.name == ability.name)
            .is_some_and(|previous_ability| ability.level > previous_ability.level);
        if leveled {
            events.push(MatchEvent::confirmed(MatchEventKind::AbilityLeveled {
                ability: ability.name.clone(),
                level: ability.level,
            }));
        }
    }
}

/// Compara a contagem de cada item entre os carregados. Mudar um item de
/// espaço (do stash para o inventário, por exemplo) não gera evento, e o
/// espaço do TP é ignorado.
fn push_inventory_changes(
    events: &mut Vec<MatchEvent>,
    previous: &GameSnapshot,
    current: &GameSnapshot,
    items: &ItemCatalog,
) {
    let (Some(previous_items), Some(current_items)) = (&previous.inventory, &current.inventory)
    else {
        return;
    };
    let previous_items = tracked(previous_items);
    let current_items = tracked(current_items);
    let previous_counts = count_by_name(&previous_items);
    let current_counts = count_by_name(&current_items);

    let added = changed_units(&current_items, &current_counts, &previous_counts);
    let mut removed = changed_units(&previous_items, &previous_counts, &current_counts);
    for item in added {
        let components = take_components(&mut removed, items.components(&item.name));
        events.push(MatchEvent::inferred(if components.is_empty() {
            MatchEventKind::ItemAdded {
                item: item.name.clone(),
                placement: ItemPlacement::of_slot(&item.slot),
            }
        } else {
            MatchEventKind::ItemAssembled {
                item: item.name.clone(),
                components,
            }
        }));
    }
    events.extend(removed.into_iter().map(|item| {
        MatchEvent::inferred(MatchEventKind::ItemRemoved {
            item: item.name.clone(),
            placement: ItemPlacement::of_slot(&item.slot),
        })
    }));
}

fn tracked(items: &[CarriedItem]) -> Vec<&CarriedItem> {
    items
        .iter()
        .filter(|item| item.slot != TELEPORT_SLOT)
        .collect()
}

/// Uma entrada por unidade a mais em `items` do que em `other`, na ordem dos
/// espaços, para a lista de eventos sair estável.
fn changed_units<'a>(
    items: &[&'a CarriedItem],
    counts: &HashMap<&str, usize>,
    other_counts: &HashMap<&str, usize>,
) -> Vec<&'a CarriedItem> {
    let mut seen = HashSet::new();
    let mut units = Vec::new();
    for item in items {
        let name = item.name.as_str();
        if seen.insert(name) {
            let extra = counts[name].saturating_sub(count_of(other_counts, name));
            units.extend(std::iter::repeat_n(*item, extra));
        }
    }
    units
}

/// Retira de `removed` uma unidade de cada componente da receita que saiu e
/// devolve os nomes retirados, na ordem da receita.
fn take_components(removed: &mut Vec<&CarriedItem>, recipe: &[String]) -> Vec<String> {
    let mut taken = Vec::new();
    for component in recipe {
        if let Some(position) = removed.iter().position(|item| &item.name == component) {
            taken.push(removed.remove(position).name.clone());
        }
    }
    taken
}

fn count_by_name<'a>(items: &[&'a CarriedItem]) -> HashMap<&'a str, usize> {
    let mut counts = HashMap::new();
    for item in items {
        *counts.entry(item.name.as_str()).or_insert(0) += 1;
    }
    counts
}

fn count_of(counts: &HashMap<&str, usize>, name: &str) -> usize {
    counts.get(name).copied().unwrap_or(0)
}

/// Resumo de uma partida terminada, montado a partir do estado do pós-jogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSummary {
    pub hero: HeroPick,
    pub result: MatchResult,
    /// Duração em segundos, pelo relógio da partida no pós-jogo.
    pub duration_secs: Option<u32>,
    pub hero_level: Option<u32>,
    pub stats: PlayerStats,
    /// Itens carregados no fim, como o GSI os enviou.
    pub final_items: Vec<CarriedItem>,
    pub abilities: Vec<AbilityState>,
    /// Só existe quando o Dota enviou um ID real.
    pub match_id: Option<MatchId>,
}

impl MatchSummary {
    /// Resumo do estado recebido no pós-jogo. Campos que o Dota não enviou
    /// ficam vazios; um relógio negativo não é uma duração.
    pub fn from_post_game(snapshot: &GameSnapshot) -> Self {
        Self {
            hero: snapshot.hero.clone(),
            result: match_result(snapshot),
            duration_secs: snapshot
                .clock_time
                .and_then(|clock_time| u32::try_from(clock_time).ok()),
            hero_level: snapshot.hero_level,
            stats: snapshot.stats.clone(),
            final_items: snapshot.inventory.clone().unwrap_or_default(),
            abilities: snapshot.abilities.clone().unwrap_or_default(),
            match_id: snapshot.match_id.clone(),
        }
    }
}

fn match_result(snapshot: &GameSnapshot) -> MatchResult {
    match (snapshot.team, snapshot.winner) {
        (Some(team), Some(winner)) if team == winner => MatchResult::Victory,
        (Some(_), Some(_)) => MatchResult::Defeat,
        _ => MatchResult::Unknown,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/match_events_tests.rs"]
mod tests;
