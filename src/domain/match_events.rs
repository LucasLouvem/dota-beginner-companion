//! Eventos da partida deduzidos da sequência de estados do GSI. Cada evento
//! informa se foi confirmado pelo GSI, inferido pelo aplicativo ou marcado
//! pelo usuário, para que uma inferência nunca apareça como fato.

use std::collections::{HashMap, HashSet};

use super::game::{CarriedItem, GamePhase, GameSnapshot, HeroPick};

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
    },
    /// Um item a menos entre os carregados: pode ter sido usado, vendido ou combinado.
    ItemRemoved {
        item: String,
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
    previous: Option<GameSnapshot>,
    started: bool,
    ended: bool,
}

impl MatchTracker {
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
            push_inventory_changes(&mut events, previous, &current);
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
/// espaço (do stash para o inventário, por exemplo) não gera evento.
fn push_inventory_changes(
    events: &mut Vec<MatchEvent>,
    previous: &GameSnapshot,
    current: &GameSnapshot,
) {
    let (Some(previous_items), Some(current_items)) = (&previous.inventory, &current.inventory)
    else {
        return;
    };
    let previous_counts = count_by_name(previous_items);
    let current_counts = count_by_name(current_items);
    // Percorre na ordem dos espaços, uma vez por nome, para a lista sair estável.
    let mut seen = HashSet::new();
    for item in current_items {
        let name = item.name.as_str();
        let added = current_counts[name].saturating_sub(count_of(&previous_counts, name));
        if seen.insert(name) {
            events.extend((0..added).map(|_| {
                MatchEvent::inferred(MatchEventKind::ItemAdded {
                    item: item.name.clone(),
                })
            }));
        }
    }
    let mut seen = HashSet::new();
    for item in previous_items {
        let name = item.name.as_str();
        let removed = previous_counts[name].saturating_sub(count_of(&current_counts, name));
        if seen.insert(name) {
            events.extend((0..removed).map(|_| {
                MatchEvent::inferred(MatchEventKind::ItemRemoved {
                    item: item.name.clone(),
                })
            }));
        }
    }
}

fn count_by_name(items: &[CarriedItem]) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for item in items {
        *counts.entry(item.name.as_str()).or_insert(0) += 1;
    }
    counts
}

fn count_of(counts: &HashMap<&str, usize>, name: &str) -> usize {
    counts.get(name).copied().unwrap_or(0)
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
