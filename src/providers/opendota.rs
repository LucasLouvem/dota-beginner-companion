//! Provider do OpenDota (`api.opendota.com/api`). Cada consulta vira um
//! caminho da API, e cada resposta é validada campo a campo: campo ausente
//! ou com tipo errado vira "sem dado", nunca valor inventado.

use std::sync::Arc;

use serde_json::{Map, Value};

use crate::application::ports::{
    BoxFuture, CallBudget, CallCategory, ProviderError, StatsProvider, StatsQuery,
};
use crate::domain::builds::{PlayerBuild, Purchase, in_purchase_order};
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::{
    FinalStats, HeroMatch, ItemPopularity, Kda, MatchDetails, MatchPlayer, PopularityPhase,
    PublicMatchSummary,
};
use crate::domain::patch::{Patch, PatchCatalog};

use super::http_json::HttpJsonClient;

/// Provider do OpenDota sobre o cliente HTTP de JSON. O catálogo de itens
/// traduz os IDs numéricos do inventário para o nome interno.
pub struct OpenDotaProvider {
    http: HttpJsonClient,
    items: Arc<ItemCatalog>,
}

impl OpenDotaProvider {
    pub fn new(http: HttpJsonClient, items: Arc<ItemCatalog>) -> Self {
        Self { http, items }
    }
}

/// Campos dos detalhes da partida guardados no cache; o resto da resposta
/// (centenas de KB, com nomes e contas dos jogadores) é descartado.
const MATCH_FIELDS: [&str; 7] = [
    "match_id",
    "leagueid",
    "game_mode",
    "patch",
    "version",
    "start_time",
    "radiant_win",
];
const PLAYER_FIELDS: [&str; 27] = [
    "hero_id",
    "win",
    "patch",
    "purchase_log",
    "ability_upgrades_arr",
    "item_0",
    "item_1",
    "item_2",
    "item_3",
    "item_4",
    "item_5",
    "backpack_0",
    "backpack_1",
    "backpack_2",
    "item_neutral",
    "item_neutral2",
    "isRadiant",
    "kills",
    "deaths",
    "assists",
    "last_hits",
    "denies",
    "gold_per_min",
    "xp_per_min",
    "net_worth",
    "obs_placed",
    "sen_placed",
];

/// Caminho da API de cada consulta.
fn path(query: &StatsQuery) -> String {
    match query {
        StatsQuery::Patches => "/constants/patch".into(),
        StatsQuery::HeroMatches { hero_id } => format!("/heroes/{hero_id}/matches"),
        StatsQuery::MatchDetails { match_id } => format!("/matches/{match_id}"),
        StatsQuery::ItemPopularity { hero_id } => format!("/heroes/{hero_id}/itemPopularity"),
        StatsQuery::PublicMatches { less_than_match_id } => match less_than_match_id {
            Some(match_id) => format!("/publicMatches?less_than_match_id={match_id}"),
            None => "/publicMatches".into(),
        },
    }
}

impl StatsProvider for OpenDotaProvider {
    fn fetch<'a>(
        &'a self,
        query: &'a StatsQuery,
        category: CallCategory,
        budget: &'a dyn CallBudget,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let path = path(query);
            let value = self.http.get_json(&path, category, budget).await?;
            let kept = match query {
                StatsQuery::MatchDetails { .. } => reduce_match(&value),
                StatsQuery::Patches
                | StatsQuery::HeroMatches { .. }
                | StatsQuery::ItemPopularity { .. }
                | StatsQuery::PublicMatches { .. } => value,
            };
            Ok(kept.to_string())
        })
    }

    fn decode_patches(&self, raw: &str) -> Result<PatchCatalog, ProviderError> {
        decode_patches(raw)
    }

    fn decode_hero_matches(&self, raw: &str) -> Result<Vec<HeroMatch>, ProviderError> {
        decode_hero_matches(raw)
    }

    fn decode_match_details(&self, raw: &str) -> Result<MatchDetails, ProviderError> {
        decode_match_details(raw, &self.items)
    }

    fn decode_public_matches(&self, raw: &str) -> Result<Vec<PublicMatchSummary>, ProviderError> {
        decode_public_matches(raw)
    }

    fn decode_item_popularity(&self, raw: &str) -> Result<ItemPopularity, ProviderError> {
        decode_item_popularity(raw, &self.items)
    }
}

/// `/constants/patch`: lista de `{name, date, id}`, espelho do dotaconstants.
/// Registros sem `id` numérico ou sem `name` ficam de fora; sem nenhum
/// registro válido, a resposta não serve.
fn decode_patches(raw: &str) -> Result<PatchCatalog, ProviderError> {
    let path = path(&StatsQuery::Patches);
    let value: Value = parse(raw, &path)?;
    let entries = value.as_array().ok_or(ProviderError::UnexpectedFormat {
        path: path.clone(),
        detail: "a lista de patches não é uma lista",
    })?;
    let patches: Vec<Patch> = entries
        .iter()
        .filter_map(|entry| {
            Some(Patch {
                id: u32::try_from(entry.get("id")?.as_u64()?).ok()?,
                name: entry.get("name")?.as_str()?.to_owned(),
            })
        })
        .collect();
    if patches.is_empty() {
        return Err(ProviderError::UnexpectedFormat {
            path,
            detail: "nenhum patch válido na resposta",
        });
    }
    Ok(PatchCatalog::new(patches))
}

/// `/heroes/{id}/matches`: lista de partidas com `match_id`, `start_time`,
/// `leagueid`, `league_name`, `radiant_win`, `radiant` e `duration`. Itens
/// sem `match_id` numérico ficam de fora.
fn decode_hero_matches(raw: &str) -> Result<Vec<HeroMatch>, ProviderError> {
    let path = "/heroes/{hero_id}/matches";
    let value = parse(raw, path)?;
    let entries = value.as_array().ok_or(ProviderError::UnexpectedFormat {
        path: path.into(),
        detail: "a lista de partidas não é uma lista",
    })?;
    Ok(entries
        .iter()
        .filter_map(|entry| {
            let radiant_win = entry.get("radiant_win").and_then(Value::as_bool);
            let radiant = entry.get("radiant").and_then(Value::as_bool);
            Some(HeroMatch {
                match_id: entry.get("match_id")?.as_u64()?,
                start_time: entry.get("start_time").and_then(Value::as_i64),
                league_id: unsigned(entry, "leagueid"),
                league_name: entry
                    .get("league_name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                hero_won: radiant_win
                    .zip(radiant)
                    .map(|(win, radiant)| win == radiant),
                duration_secs: unsigned(entry, "duration"),
                kda: kda(entry),
            })
        })
        .collect())
}

/// `/heroes/{id}/itemPopularity`: um objeto por fase (`start_game_items`,
/// `early_game_items`, `mid_game_items`, `late_game_items`), com o ID do
/// item em texto como chave e a contagem como valor. ID desconhecido fica
/// de fora; fase ausente fica vazia.
fn decode_item_popularity(raw: &str, items: &ItemCatalog) -> Result<ItemPopularity, ProviderError> {
    let path = "/heroes/{hero_id}/itemPopularity";
    let value = parse(raw, path)?;
    if !value.is_object() {
        return Err(ProviderError::UnexpectedFormat {
            path: path.into(),
            detail: "a popularidade de itens não é um objeto",
        });
    }
    let phases = [
        (PopularityPhase::Start, "start_game_items"),
        (PopularityPhase::Early, "early_game_items"),
        (PopularityPhase::Mid, "mid_game_items"),
        (PopularityPhase::Late, "late_game_items"),
    ]
    .into_iter()
    .map(|(phase, field)| {
        let mut counts: Vec<(String, u32)> = value
            .get(field)
            .and_then(Value::as_object)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(id, count)| {
                        let item = items.by_id(id.parse().ok()?)?;
                        Some((item.name.clone(), u32::try_from(count.as_u64()?).ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Mais comprado primeiro; empate pelo nome, para a ordem ser estável.
        counts.sort_by(|first, second| second.1.cmp(&first.1).then(first.0.cmp(&second.0)));
        (phase, counts)
    })
    .collect();
    Ok(ItemPopularity { phases })
}

/// `/publicMatches`: lista com `match_id`, `game_mode`, `lobby_type`,
/// `avg_rank_tier` e `start_time`. Os heróis dos times vêm zerados na
/// resposta real e não são lidos. Itens sem `match_id` numérico ficam de fora.
fn decode_public_matches(raw: &str) -> Result<Vec<PublicMatchSummary>, ProviderError> {
    let path = "/publicMatches";
    let value = parse(raw, path)?;
    let entries = value.as_array().ok_or(ProviderError::UnexpectedFormat {
        path: path.into(),
        detail: "a lista de partidas públicas não é uma lista",
    })?;
    Ok(entries
        .iter()
        .filter_map(|entry| {
            Some(PublicMatchSummary {
                match_id: entry.get("match_id")?.as_u64()?,
                game_mode: unsigned(entry, "game_mode"),
                lobby_type: unsigned(entry, "lobby_type"),
                avg_rank_tier: unsigned(entry, "avg_rank_tier"),
                start_time: entry.get("start_time").and_then(Value::as_i64),
            })
        })
        .collect())
}

/// `/matches/{id}` (ou a versão reduzida do cache). Sem `match_id` ou sem a
/// lista de jogadores, a resposta não serve.
fn decode_match_details(raw: &str, items: &ItemCatalog) -> Result<MatchDetails, ProviderError> {
    let path = "/matches/{match_id}";
    let value = parse(raw, path)?;
    let unexpected = |detail| ProviderError::UnexpectedFormat {
        path: path.into(),
        detail,
    };
    let match_id = value
        .get("match_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| unexpected("partida sem match_id"))?;
    let players = value
        .get("players")
        .and_then(Value::as_array)
        .ok_or_else(|| unexpected("partida sem a lista de jogadores"))?;
    Ok(MatchDetails {
        match_id,
        league_id: unsigned(&value, "leagueid"),
        game_mode: unsigned(&value, "game_mode"),
        patch_id: unsigned(&value, "patch"),
        parsed: value
            .get("version")
            .is_some_and(|version| !version.is_null()),
        start_time: value.get("start_time").and_then(Value::as_i64),
        players: players
            .iter()
            .filter_map(|player| decode_player(player, items))
            .collect(),
    })
}

fn decode_player(player: &Value, items: &ItemCatalog) -> Option<MatchPlayer> {
    Some(MatchPlayer {
        hero_id: unsigned(player, "hero_id")?,
        // `win` é 0 ou 1 na fonte.
        won: player
            .get("win")
            .and_then(Value::as_u64)
            .map(|win| win == 1),
        patch_id: unsigned(player, "patch"),
        kda: kda(player),
        stats: FinalStats {
            last_hits: unsigned(player, "last_hits"),
            denies: unsigned(player, "denies"),
            gpm: unsigned(player, "gold_per_min"),
            xpm: unsigned(player, "xp_per_min"),
            net_worth: unsigned(player, "net_worth"),
            observers_placed: unsigned(player, "obs_placed"),
            sentries_placed: unsigned(player, "sen_placed"),
        },
        build: player_build(player, items),
        ability_upgrades: player
            .get("ability_upgrades_arr")
            .and_then(Value::as_array)
            .map(|upgrades| {
                upgrades
                    .iter()
                    .filter_map(|id| u32::try_from(id.as_u64()?).ok())
                    .collect()
            }),
    })
}

/// Build real do jogador. O inventário (`item_0..5`) é obrigatório: sem
/// nenhum desses campos, a fonte não trouxe a build. Mochila, neutro e
/// compras ausentes ficam vazios.
fn player_build(player: &Value, items: &ItemCatalog) -> Option<PlayerBuild> {
    let has_inventory = (0..6).any(|slot| player.get(format!("item_{slot}")).is_some());
    if !has_inventory {
        return None;
    }
    let slots = |prefix: &str, count: usize| -> Vec<String> {
        (0..count)
            .filter_map(|slot| item_in(player, &format!("{prefix}{slot}"), items))
            .collect()
    };
    Some(PlayerBuild {
        inventory: slots("item_", 6),
        backpack: slots("backpack_", 3),
        neutral: item_in(player, "item_neutral", items),
        neutral_enhancement: item_in(player, "item_neutral2", items),
        purchases: player
            .get("purchase_log")
            .and_then(Value::as_array)
            .map(|log| {
                in_purchase_order(
                    log.iter()
                        .filter_map(|purchase| {
                            Some(Purchase {
                                // A chave vem sem o prefixo usado pelo GSI.
                                item: format!("item_{}", purchase.get("key")?.as_str()?),
                                time: i32::try_from(purchase.get("time")?.as_i64()?).ok()?,
                            })
                        })
                        .collect(),
                )
            }),
    })
}

/// Item de um espaço pelo catálogo: 0 é espaço vazio, e ID desconhecido
/// fica de fora.
fn item_in(player: &Value, field: &str, items: &ItemCatalog) -> Option<String> {
    let id = unsigned(player, field).filter(|id| *id != 0)?;
    items.by_id(id).map(|item| item.name.clone())
}

/// Mantém só os campos usados da partida e dos jogadores.
fn reduce_match(value: &Value) -> Value {
    let keep = |source: &Value, fields: &[&str]| -> Map<String, Value> {
        fields
            .iter()
            .filter_map(|field| Some(((*field).to_owned(), source.get(*field)?.clone())))
            .collect()
    };
    let mut reduced = keep(value, &MATCH_FIELDS);
    if let Some(players) = value.get("players").and_then(Value::as_array) {
        reduced.insert(
            "players".into(),
            Value::Array(
                players
                    .iter()
                    .map(|player| Value::Object(keep(player, &PLAYER_FIELDS)))
                    .collect(),
            ),
        );
    }
    Value::Object(reduced)
}

/// Abates, mortes e assistências; sem os três números, "sem dado".
fn kda(value: &Value) -> Option<Kda> {
    Some(Kda {
        kills: unsigned(value, "kills")?,
        deaths: unsigned(value, "deaths")?,
        assists: unsigned(value, "assists")?,
    })
}

/// Inteiro sem sinal que cabe em `u32`; outro tipo ou valor é "sem dado".
fn unsigned(value: &Value, field: &str) -> Option<u32> {
    u32::try_from(value.get(field)?.as_u64()?).ok()
}

fn parse(raw: &str, path: &str) -> Result<Value, ProviderError> {
    serde_json::from_str(raw).map_err(|json_error| ProviderError::InvalidJson {
        path: path.into(),
        source: std::sync::Arc::new(json_error),
    })
}

#[cfg(test)]
#[path = "../../tests/unit/providers/opendota_tests.rs"]
mod tests;
