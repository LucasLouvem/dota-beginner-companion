//! Dados de partidas vindos de uma fonte de estatísticas (o OpenDota), já
//! validados: campo ausente ou com tipo errado vira `None`, nunca valor
//! inventado. Os itens já estão com o nome interno (`item_black_king_bar`).

use super::builds::{PlayerMatch, Purchase};

/// Partida recente com um herói, da lista de candidatas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroMatch {
    pub match_id: u64,
    /// Segundos desde 1970.
    pub start_time: Option<i64>,
    /// Liga da partida; ausente ou zero quando não é de liga.
    pub league_id: Option<u32>,
    pub league_name: Option<String>,
    /// Se o time do herói venceu.
    pub hero_won: Option<bool>,
    pub duration_secs: Option<u32>,
}

impl HeroMatch {
    /// Partida de liga (profissional): `leagueid` presente e diferente de zero.
    pub fn is_league_match(&self) -> bool {
        self.league_id.is_some_and(|league_id| league_id != 0)
    }
}

/// Detalhes de uma partida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchDetails {
    pub match_id: u64,
    pub league_id: Option<u32>,
    pub game_mode: Option<u32>,
    /// Patch no topo da partida; o de cada jogador tem prioridade.
    pub patch_id: Option<u32>,
    /// A partida foi analisada pela fonte (tem `version`), o que traz o
    /// registro de compras e a ordem de habilidades.
    pub parsed: bool,
    pub start_time: Option<i64>,
    pub players: Vec<MatchPlayer>,
}

impl MatchDetails {
    pub fn is_league_match(&self) -> bool {
        self.league_id.is_some_and(|league_id| league_id != 0)
    }

    /// Jogador com o herói, se ele estava na partida.
    pub fn player_with_hero(&self, hero_id: u32) -> Option<&MatchPlayer> {
        self.players.iter().find(|player| player.hero_id == hero_id)
    }

    /// Patch do jogador: o campo dele, ou o do topo da partida. Nunca a
    /// versão da análise.
    pub fn patch_of(&self, player: &MatchPlayer) -> Option<u32> {
        player.patch_id.or(self.patch_id)
    }
}

/// Um jogador nos detalhes da partida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPlayer {
    pub hero_id: u32,
    pub won: Option<bool>,
    pub patch_id: Option<u32>,
    /// Ausente em partida não analisada.
    pub purchase_log: Option<Vec<Purchase>>,
    /// Itens de `item_0..5` conhecidos pelo catálogo, na ordem dos espaços;
    /// ausente quando a fonte não traz os espaços.
    pub final_items: Option<Vec<String>>,
    /// IDs das habilidades na ordem em que foram evoluídas.
    pub ability_upgrades: Option<Vec<u32>>,
}

impl MatchPlayer {
    /// Dados para montar a build, quando há resultado e itens finais.
    pub fn build_input(&self) -> Option<PlayerMatch> {
        Some(PlayerMatch {
            won: self.won?,
            purchase_log: self.purchase_log.clone(),
            final_items: self.final_items.clone()?,
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/match_data_tests.rs"]
mod tests;
