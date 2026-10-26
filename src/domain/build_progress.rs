//! Leitura de uma build real para acompanhar a partida: a ordem dos itens
//! completos, os itens iniciais, as compras por janela de tempo e o progresso
//! do jogador nos itens e nos pontos de habilidade. Só usa o que a build e o
//! GSI informam; nada é estimado.

use std::collections::HashSet;

use super::builds::{PlayerBuild, Purchase};
use super::game::GameSnapshot;
use super::item_catalog::ItemCatalog;

/// Fim da primeira janela de compras (10 minutos), como na popularidade de
/// itens do OpenDota.
const EARLY_WINDOW_END: i32 = 10 * 60;
/// Fim da segunda janela de compras (25 minutos).
const MID_WINDOW_END: i32 = 25 * 60;
/// Profundidade máxima das receitas seguidas ao expandir componentes; protege
/// contra um ciclo nos dados embutidos.
const MAX_RECIPE_DEPTH: usize = 8;

/// De onde vem a ordem dos itens da build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemOrderEvidence {
    /// Ordem da primeira compra de cada item no `purchase_log`.
    PurchaseLog,
    /// A partida não tem registro de compras: vale a ordem dos espaços.
    InventorySlots,
}

/// Itens completos da build, na ordem em que foram comprados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildItemOrder {
    pub evidence: ItemOrderEvidence,
    /// Nomes internos, sem repetição.
    pub items: Vec<String>,
}

/// Itens do fim da partida (inventário e mochila, sem o neutro), na ordem da
/// primeira compra. Um item sem registro de compra (como a Aegis ou um item
/// recebido) fica no fim, na ordem dos espaços.
pub fn build_item_order(build: &PlayerBuild) -> BuildItemOrder {
    let mut finals: Vec<&String> = Vec::new();
    for item in build.inventory.iter().chain(&build.backpack) {
        if !finals.contains(&item) {
            finals.push(item);
        }
    }
    let Some(purchases) = &build.purchases else {
        return BuildItemOrder {
            evidence: ItemOrderEvidence::InventorySlots,
            items: finals.into_iter().cloned().collect(),
        };
    };
    let first_purchase = |item: &str| purchases.iter().position(|purchase| purchase.item == item);
    let mut ordered: Vec<(Option<usize>, usize, &String)> = finals
        .into_iter()
        .enumerate()
        .map(|(slot, item)| (first_purchase(item), slot, item))
        .collect();
    // Comprados primeiro, pela posição da compra; sem compra, pelos espaços.
    ordered.sort_by_key(|(purchase, slot, _)| (purchase.is_none(), *purchase, *slot));
    BuildItemOrder {
        evidence: ItemOrderEvidence::PurchaseLog,
        items: ordered
            .into_iter()
            .map(|(_, _, item)| item.clone())
            .collect(),
    }
}

/// Compras feitas antes do início da partida (horário ≤ 0).
pub fn starting_items(build: &PlayerBuild) -> Vec<Purchase> {
    build
        .purchases
        .iter()
        .flatten()
        .filter(|purchase| purchase.time <= 0)
        .cloned()
        .collect()
}

/// Janela de tempo de uma compra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PurchaseWindow {
    BeforeStart,
    /// Até 10 minutos.
    Early,
    /// De 10 a 25 minutos.
    Mid,
    /// Depois de 25 minutos.
    Late,
}

impl PurchaseWindow {
    pub fn of(time: i32) -> Self {
        match time {
            ..=0 => Self::BeforeStart,
            1..EARLY_WINDOW_END => Self::Early,
            EARLY_WINDOW_END..MID_WINDOW_END => Self::Mid,
            _ => Self::Late,
        }
    }
}

/// Todas as compras, agrupadas por janela e na ordem original; só aparecem
/// as janelas com compras. Vazio sem registro de compras.
pub fn purchases_by_window(build: &PlayerBuild) -> Vec<(PurchaseWindow, Vec<Purchase>)> {
    let mut windows: Vec<(PurchaseWindow, Vec<Purchase>)> = Vec::new();
    for purchase in build.purchases.iter().flatten() {
        let window = PurchaseWindow::of(purchase.time);
        match windows.last_mut() {
            Some((last, purchases)) if *last == window => purchases.push(purchase.clone()),
            _ => windows.push((window, vec![purchase.clone()])),
        }
    }
    windows
}

/// Situação de um passo da build em relação à partida atual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    /// O primeiro passo ainda não feito.
    Next,
    Pending,
}

/// Um item da ordem da build e se o jogador já o tem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStep {
    pub item: String,
    pub state: StepState,
}

/// Itens que o jogador tem no estado do GSI (inventário, mochila,
/// esconderijo, TP e neutro) e os componentes de cada um, pelas receitas do
/// catálogo: um item montado conta como compra dos componentes.
pub fn held_items(game: &GameSnapshot, catalog: &ItemCatalog) -> HashSet<String> {
    let mut held = HashSet::new();
    let mut pending: Vec<(String, usize)> = game
        .inventory
        .iter()
        .flatten()
        .map(|carried| (carried.name.clone(), 0))
        .collect();
    while let Some((item, depth)) = pending.pop() {
        if depth < MAX_RECIPE_DEPTH {
            pending.extend(
                catalog
                    .components(&item)
                    .iter()
                    .map(|component| (component.clone(), depth + 1)),
            );
        }
        held.insert(item);
    }
    held
}

/// Cada item da ordem marcado como feito, próximo ou pendente.
pub fn item_progress(order: &[String], held: &HashSet<String>) -> Vec<ItemStep> {
    let mut next_marked = false;
    order
        .iter()
        .map(|item| {
            let state = if held.contains(item) {
                StepState::Done
            } else if next_marked {
                StepState::Pending
            } else {
                next_marked = true;
                StepState::Next
            };
            ItemStep {
                item: item.clone(),
                state,
            }
        })
        .collect()
}

/// Pontos de habilidade já gastos no estado do GSI: a soma dos níveis das
/// habilidades (sem as cosméticas) e os talentos escolhidos. `None` enquanto
/// o Dota não envia as habilidades.
pub fn points_spent(game: &GameSnapshot) -> Option<usize> {
    let abilities = game.abilities.as_ref()?;
    let levels: u32 = abilities
        .iter()
        .filter(|ability| !ability.is_cosmetic())
        .map(|ability| ability.level)
        .sum();
    Some((levels + game.talents_learned.unwrap_or_default()) as usize)
}

/// Cada ponto da ordem real de habilidades marcado pelos pontos já gastos
/// na partida; sem essa informação, todos ficam pendentes.
pub fn ability_progress(upgrades: &[u32], spent: Option<usize>) -> Vec<(u32, StepState)> {
    upgrades
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let state = match spent {
                Some(spent) if index < spent => StepState::Done,
                Some(spent) if index == spent => StepState::Next,
                _ => StepState::Pending,
            };
            (*id, state)
        })
        .collect()
}

/// Habilidades na ordem em que receberam o último ponto nesta partida, sem
/// os talentos (`is_talent`). Descreve a partida, não uma regra geral.
pub fn ability_max_order(upgrades: &[u32], is_talent: impl Fn(u32) -> bool) -> Vec<u32> {
    let mut last_point: Vec<(usize, u32)> = Vec::new();
    for (index, id) in upgrades.iter().enumerate() {
        if is_talent(*id) {
            continue;
        }
        match last_point.iter_mut().find(|(_, seen)| seen == id) {
            Some(entry) => entry.0 = index,
            None => last_point.push((index, *id)),
        }
    }
    last_point.sort_by_key(|(index, _)| *index);
    last_point.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
#[path = "../../tests/unit/domain/build_progress_tests.rs"]
mod tests;
