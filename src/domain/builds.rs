//! Builds e ordem de habilidades a partir dos dados de partidas do OpenDota.
//! A build é uma aproximação feita com os dados disponíveis: os três
//! primeiros itens grandes de cada partida, não um evento informado pelo jogo.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::item_catalog::ItemCatalog;

/// Quantidade de itens grandes que formam uma build.
pub const BUILD_SIZE: usize = 3;
/// Quantidade mínima de partidas para uma build ou uma ordem de habilidades
/// ser mostrada.
pub const MIN_SAMPLE: usize = 10;
/// Evoluções de habilidade comparadas na ordem de habilidades (níveis 1 a
/// 10). A sequência inteira quase nunca é igual entre partidas.
pub const ABILITY_ORDER_LEVELS: usize = 10;

/// De onde vem a ordem dos itens da build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuildEvidence {
    /// Ordem do primeiro registro de compra de cada item (`purchase_log`).
    OrderedPurchaseLog,
    /// Itens do inventário final, sem ordem de compra.
    FinalInventoryUnordered,
}

/// Registro de compra de uma partida analisada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purchase {
    /// Nome interno, como o GSI usa (`item_black_king_bar`).
    pub item: String,
    /// Segundos de partida; negativo antes do início.
    pub time: i32,
}

/// Dados de um jogador numa partida, já validados pelo provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerMatch {
    pub won: bool,
    /// Ausente em partida não analisada.
    pub purchase_log: Option<Vec<Purchase>>,
    /// Itens de `item_0..5`, na ordem dos espaços, sem os espaços vazios.
    pub final_items: Vec<String>,
}

/// Identifica uma build: itens em ordem de compra ou, sem ordem, em ordem
/// alfabética. Builds de evidências diferentes nunca são iguais.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuildKey {
    pub evidence: BuildEvidence,
    pub items: Vec<String>,
}

/// Build de uma partida, com o momento de cada item quando há ordem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchBuild {
    pub key: BuildKey,
    /// Segundos do primeiro registro de cada item; só com `OrderedPurchaseLog`.
    pub times: Option<Vec<i32>>,
}

/// Build agregada de várias partidas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedBuild {
    pub key: BuildKey,
    pub matches: usize,
    pub wins: usize,
    /// Momento médio de cada item, em segundos; só com `OrderedPurchaseLog`.
    pub average_times: Option<Vec<i32>>,
}

/// Builds com amostra suficiente, da maior para a menor taxa de vitória
/// observada, e quantas partidas foram analisadas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildRanking {
    pub builds: Vec<RankedBuild>,
    /// Partidas que formaram build.
    pub analyzed: usize,
}

/// Ordem de habilidades mais comum, com a amostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityOrder {
    /// IDs das habilidades, do nível 1 ao `ABILITY_ORDER_LEVELS`.
    pub ability_ids: Vec<u32>,
    /// Partidas com essa ordem.
    pub matches: usize,
    /// Partidas com evoluções suficientes para comparar.
    pub analyzed: usize,
}

/// Build da partida, ou `None` quando ela não tem `BUILD_SIZE` itens grandes.
///
/// Com `purchase_log` não vazio, vale o primeiro registro de cada item
/// grande, em ordem de horário (componentes não são itens grandes e ficam
/// de fora). Sem ele, valem os primeiros itens grandes do inventário final,
/// sem ordem.
pub fn build_from_match(player: &PlayerMatch, catalog: &ItemCatalog) -> Option<MatchBuild> {
    match player.purchase_log.as_deref() {
        Some(log) if !log.is_empty() => ordered_build(log, catalog),
        _ => unordered_build(&player.final_items, catalog),
    }
}

fn ordered_build(log: &[Purchase], catalog: &ItemCatalog) -> Option<MatchBuild> {
    let mut purchases: Vec<&Purchase> = log
        .iter()
        .filter(|purchase| catalog.is_major_item(&purchase.item))
        .collect();
    // Ordenação estável: registros com o mesmo horário mantêm a ordem da fonte.
    purchases.sort_by_key(|purchase| purchase.time);
    let mut first: Vec<&Purchase> = Vec::new();
    for purchase in purchases {
        if !first.iter().any(|seen| seen.item == purchase.item) {
            first.push(purchase);
        }
        if first.len() == BUILD_SIZE {
            break;
        }
    }
    (first.len() == BUILD_SIZE).then(|| MatchBuild {
        key: BuildKey {
            evidence: BuildEvidence::OrderedPurchaseLog,
            items: first.iter().map(|purchase| purchase.item.clone()).collect(),
        },
        times: Some(first.iter().map(|purchase| purchase.time).collect()),
    })
}

fn unordered_build(final_items: &[String], catalog: &ItemCatalog) -> Option<MatchBuild> {
    let mut items: Vec<String> = Vec::new();
    for item in final_items {
        if catalog.is_major_item(item) && !items.contains(item) {
            items.push(item.clone());
        }
        if items.len() == BUILD_SIZE {
            break;
        }
    }
    if items.len() < BUILD_SIZE {
        return None;
    }
    items.sort();
    Some(MatchBuild {
        key: BuildKey {
            evidence: BuildEvidence::FinalInventoryUnordered,
            items,
        },
        times: None,
    })
}

/// Agrupa as builds iguais, mantém as com pelo menos `min_sample` partidas e
/// devolve até `limit`, da maior para a menor taxa de vitória observada.
/// Empate na taxa: a de amostra maior vem antes; depois, a ordem da chave,
/// para o resultado não depender da ordem de entrada. É uma métrica simples:
/// não considera a incerteza de amostras pequenas além do mínimo.
pub fn rank_builds(
    builds: impl IntoIterator<Item = (MatchBuild, bool)>,
    min_sample: usize,
    limit: usize,
) -> BuildRanking {
    struct Group {
        matches: usize,
        wins: usize,
        time_sums: Option<Vec<i64>>,
    }
    let mut groups: HashMap<BuildKey, Group> = HashMap::new();
    let mut analyzed = 0;
    for (build, won) in builds {
        analyzed += 1;
        let group = groups.entry(build.key).or_insert_with(|| Group {
            matches: 0,
            wins: 0,
            time_sums: build.times.as_ref().map(|times| vec![0; times.len()]),
        });
        group.matches += 1;
        group.wins += usize::from(won);
        if let (Some(sums), Some(times)) = (&mut group.time_sums, &build.times) {
            for (sum, time) in sums.iter_mut().zip(times) {
                *sum += i64::from(*time);
            }
        }
    }
    let mut ranked: Vec<RankedBuild> = groups
        .into_iter()
        .filter(|(_, group)| group.matches >= min_sample)
        .map(|(key, group)| RankedBuild {
            key,
            matches: group.matches,
            wins: group.wins,
            average_times: group.time_sums.map(|sums| {
                sums.iter()
                    .map(|sum| (sum / group.matches as i64) as i32)
                    .collect()
            }),
        })
        .collect();
    ranked.sort_by(compare_ranked);
    ranked.truncate(limit);
    BuildRanking {
        builds: ranked,
        analyzed,
    }
}

/// Maior taxa primeiro, comparada sem ponto flutuante:
/// `wins_a / matches_a > wins_b / matches_b` ⇔ `wins_a * matches_b > wins_b * matches_a`.
fn compare_ranked(first: &RankedBuild, second: &RankedBuild) -> Ordering {
    let first_rate = first.wins * second.matches;
    let second_rate = second.wins * first.matches;
    second_rate
        .cmp(&first_rate)
        .then(second.matches.cmp(&first.matches))
        .then(first.key.cmp(&second.key))
}

/// Ordem de habilidades mais comum nos primeiros `ABILITY_ORDER_LEVELS`
/// níveis, entre as partidas com pelo menos essa quantidade de evoluções.
/// `None` quando nenhuma ordem chega a `min_sample` partidas.
pub fn ability_order(upgrades: &[Vec<u32>], min_sample: usize) -> Option<AbilityOrder> {
    let mut counts: HashMap<&[u32], usize> = HashMap::new();
    let mut analyzed = 0;
    for sequence in upgrades {
        if let Some(prefix) = sequence.get(..ABILITY_ORDER_LEVELS) {
            analyzed += 1;
            *counts.entry(prefix).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, matches)| *matches >= min_sample)
        // Mais partidas primeiro; empate decidido pela sequência, para o
        // resultado não depender da ordem do HashMap.
        .max_by(|(first, first_count), (second, second_count)| {
            first_count.cmp(second_count).then(second.cmp(first))
        })
        .map(|(prefix, matches)| AbilityOrder {
            ability_ids: prefix.to_vec(),
            matches,
            analyzed,
        })
}

#[cfg(test)]
#[path = "../../tests/unit/domain/builds_tests.rs"]
mod tests;
