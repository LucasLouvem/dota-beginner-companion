//! Converte o JSON de um evento do GSI no estado de jogo do domínio. Cada
//! campo é lido isoladamente: um campo ausente ou com tipo inesperado vira
//! "sem dado", sem descartar o resto do evento.

use serde_json::{Map, Value};

use crate::domain::game::{
    AbilityState, CarriedItem, GamePhase, GameSnapshot, HeroPick, MatchId, PlayerStats, Team,
};

/// Espaços do inventário considerados itens do jogador, na ordem exibida.
/// `preserved_neutral*` fica de fora: guarda itens neutros antigos, que não
/// estão em uso.
const CARRIED_SLOTS: &[&str] = &[
    "slot0",
    "slot1",
    "slot2",
    "slot3",
    "slot4",
    "slot5",
    "slot6",
    "slot7",
    "slot8",
    "stash0",
    "stash1",
    "stash2",
    "stash3",
    "stash4",
    "stash5",
    "teleport0",
    "neutral0",
    "neutral1",
];

/// Nome que o GSI usa para um espaço vazio.
const EMPTY_SLOT: &str = "empty";

/// Estado de jogo do payload (já sem o bloco `auth`).
pub fn game_snapshot(payload: &Value) -> GameSnapshot {
    let map = payload.get("map").and_then(Value::as_object);
    let hero = payload.get("hero").and_then(Value::as_object);
    let player = payload.get("player").and_then(Value::as_object);
    GameSnapshot {
        phase: map
            .and_then(|map| text(map, "game_state"))
            .map(GamePhase::from_gsi),
        clock_time: map.and_then(|map| signed_number(map, "clock_time")),
        match_id: map
            .and_then(|map| text(map, "matchid"))
            .and_then(MatchId::from_gsi),
        hero: hero.map_or(HeroPick::NotPicked, hero_pick),
        hero_level: hero.and_then(|hero| number(hero, "level")),
        hero_alive: hero
            .and_then(|hero| hero.get("alive"))
            .and_then(Value::as_bool),
        team: player
            .and_then(|player| text(player, "team_name"))
            .and_then(Team::from_gsi),
        winner: map
            .and_then(|map| text(map, "win_team"))
            .and_then(Team::from_gsi),
        stats: player.map(player_stats).unwrap_or_default(),
        abilities: non_empty_object(payload, "abilities").map(abilities),
        inventory: non_empty_object(payload, "items").map(inventory),
    }
}

/// O herói só conta como escolhido quando tem nome; antes da escolha o GSI
/// envia apenas `id: 0`.
fn hero_pick(hero: &Map<String, Value>) -> HeroPick {
    match text(hero, "name") {
        Some(name) if !name.is_empty() => HeroPick::Picked {
            name: name.to_owned(),
            id: number(hero, "id").filter(|id| *id != 0),
        },
        _ => HeroPick::NotPicked,
    }
}

fn player_stats(player: &Map<String, Value>) -> PlayerStats {
    PlayerStats {
        kills: number(player, "kills"),
        deaths: number(player, "deaths"),
        assists: number(player, "assists"),
        last_hits: number(player, "last_hits"),
        denies: number(player, "denies"),
        gold: number(player, "gold"),
        gpm: number(player, "gpm"),
        xpm: number(player, "xpm"),
    }
}

/// Habilidades `ability0..N` em ordem de índice. Entradas sem nome são ignoradas.
fn abilities(abilities: &Map<String, Value>) -> Vec<AbilityState> {
    let mut indexed: Vec<(u32, AbilityState)> = abilities
        .iter()
        .filter_map(|(key, value)| {
            let index = key.strip_prefix("ability")?.parse().ok()?;
            let ability = value.as_object()?;
            Some((
                index,
                AbilityState {
                    name: text(ability, "name")?.to_owned(),
                    level: number(ability, "level").unwrap_or_default(),
                    ultimate: ability
                        .get("ultimate")
                        .and_then(Value::as_bool)
                        .unwrap_or_default(),
                },
            ))
        })
        .collect();
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, ability)| ability).collect()
}

fn inventory(items: &Map<String, Value>) -> Vec<CarriedItem> {
    CARRIED_SLOTS
        .iter()
        .filter_map(|slot| {
            let slot_contents = items.get(*slot).and_then(Value::as_object)?;
            let name = text(slot_contents, "name").filter(|name| *name != EMPTY_SLOT)?;
            Some(CarriedItem {
                slot: (*slot).to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// Bloco presente e com conteúdo. O GSI envia `{}` quando ainda não tem os
/// dados, como os itens durante o tempo de estratégia.
fn non_empty_object<'a>(payload: &'a Value, key: &str) -> Option<&'a Map<String, Value>> {
    payload
        .get(key)
        .and_then(Value::as_object)
        .filter(|block| !block.is_empty())
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

fn number(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn signed_number(object: &Map<String, Value>, key: &str) -> Option<i32> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

#[cfg(test)]
#[path = "../../tests/unit/gsi/normalize_tests.rs"]
mod tests;
