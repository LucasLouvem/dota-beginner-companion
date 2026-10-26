//! Estado de jogo do jogador local, normalizado a partir de um evento do GSI.
//! Todos os campos são opcionais: o Dota só envia o que existe em cada fase,
//! e um campo ausente quer dizer "sem dado", não "zero".

/// Fase da partida (`map.game_state` do GSI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    /// Espera os jogadores carregarem; quando aparece, vem antes da seleção.
    WaitForPlayersToLoad,
    HeroSelection,
    StrategyTime,
    TeamShowcase,
    WaitForMapToLoad,
    PreGame,
    InProgress,
    PostGame,
    /// Estado que o aplicativo não trata, guardado como veio do GSI.
    Other(String),
}

impl GamePhase {
    /// Converte o valor de `map.game_state` (ex.: `DOTA_GAMERULES_STATE_PRE_GAME`).
    pub fn from_gsi(game_state: &str) -> Self {
        match game_state {
            "DOTA_GAMERULES_STATE_WAIT_FOR_PLAYERS_TO_LOAD" => GamePhase::WaitForPlayersToLoad,
            "DOTA_GAMERULES_STATE_HERO_SELECTION" => GamePhase::HeroSelection,
            "DOTA_GAMERULES_STATE_STRATEGY_TIME" => GamePhase::StrategyTime,
            "DOTA_GAMERULES_STATE_TEAM_SHOWCASE" => GamePhase::TeamShowcase,
            "DOTA_GAMERULES_STATE_WAIT_FOR_MAP_TO_LOAD" => GamePhase::WaitForMapToLoad,
            "DOTA_GAMERULES_STATE_PRE_GAME" => GamePhase::PreGame,
            "DOTA_GAMERULES_STATE_GAME_IN_PROGRESS" => GamePhase::InProgress,
            "DOTA_GAMERULES_STATE_POST_GAME" => GamePhase::PostGame,
            other => GamePhase::Other(other.to_owned()),
        }
    }

    /// Posição da fase na sequência observada da partida; `None` para estados
    /// que não fazem parte dela.
    pub fn sequence_position(&self) -> Option<u8> {
        match self {
            GamePhase::WaitForPlayersToLoad => Some(0),
            GamePhase::HeroSelection => Some(1),
            GamePhase::StrategyTime => Some(2),
            GamePhase::TeamShowcase => Some(3),
            GamePhase::WaitForMapToLoad => Some(4),
            GamePhase::PreGame => Some(5),
            GamePhase::InProgress => Some(6),
            GamePhase::PostGame => Some(7),
            GamePhase::Other(_) => None,
        }
    }
}

/// Lado do mapa (`player.team_name` e `map.win_team`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Radiant,
    Dire,
}

impl Team {
    /// `radiant` ou `dire`; qualquer outro valor (como `none`) é ausência de time.
    pub fn from_gsi(team_name: &str) -> Option<Self> {
        match team_name {
            "radiant" => Some(Team::Radiant),
            "dire" => Some(Team::Dire),
            _ => None,
        }
    }
}

/// Modo da partida. O GSI não informa o modo; ele vem da escolha do usuário.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GameMode {
    /// Escolha Livre.
    #[default]
    AllPick,
    Turbo,
    /// Contra bots.
    Bots,
}

impl GameMode {
    /// Todos os modos, na ordem em que são oferecidos ao usuário.
    pub const ALL: [GameMode; 3] = [GameMode::AllPick, GameMode::Turbo, GameMode::Bots];
}

/// ID da partida no Dota (`map.matchid`). Contra bots o GSI envia `"0"`
/// durante a partida; o ID real só chega no pós-jogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchId(String);

impl MatchId {
    /// `None` para `"0"`, texto vazio ou valor que não seja só dígitos.
    pub fn from_gsi(value: &str) -> Option<Self> {
        let is_real_id = !value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && value.bytes().any(|byte| byte != b'0');
        is_real_id.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Herói do jogador. Antes da escolha o GSI envia `hero: {"id": 0}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeroPick {
    NotPicked,
    Picked {
        /// Nome interno, como `npc_dota_hero_zuus`.
        name: String,
        id: Option<u32>,
    },
}

/// Habilidade do herói com o nível atual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityState {
    /// Nome interno, como `zuus_arc_lightning`.
    pub name: String,
    pub level: u32,
    pub ultimate: bool,
}

impl AbilityState {
    /// Habilidade cosmética do Dota Plus (`plus_*`, como `plus_high_five`),
    /// que o GSI envia junto com as do herói em partidas com matchmaking.
    pub fn is_cosmetic(&self) -> bool {
        self.name.starts_with("plus_")
    }
}

/// Item em um espaço do inventário (`slot0..8`, `stash0..5`, `teleport0`,
/// `neutral0..1`). Espaços vazios não entram na lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedItem {
    pub slot: String,
    /// Nome interno, como `item_tango`.
    pub name: String,
}

/// Números do jogador enviados pelo GSI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerStats {
    pub kills: Option<u32>,
    pub deaths: Option<u32>,
    pub assists: Option<u32>,
    pub last_hits: Option<u32>,
    pub denies: Option<u32>,
    pub gold: Option<u32>,
    pub gpm: Option<u32>,
    pub xpm: Option<u32>,
}

/// Estado de jogo de um evento do GSI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSnapshot {
    /// `None` quando o evento não tem o bloco `map`, como no menu do Dota.
    pub phase: Option<GamePhase>,
    /// Relógio da partida em segundos (`map.clock_time`). É negativo antes do
    /// início e volta entre fases; não serve para ordenar eventos.
    pub clock_time: Option<i32>,
    /// `None` enquanto o Dota não informa um ID real.
    pub match_id: Option<MatchId>,
    pub hero: HeroPick,
    pub hero_level: Option<u32>,
    /// Talentos já escolhidos (`hero.talent_1..8` verdadeiros); `None` quando
    /// o Dota não envia os talentos.
    pub talents_learned: Option<u32>,
    pub hero_alive: Option<bool>,
    pub team: Option<Team>,
    /// Time vencedor; só existe depois que o Dota informa `win_team`.
    pub winner: Option<Team>,
    pub stats: PlayerStats,
    /// `None` quando o Dota ainda não envia habilidades (antes do pré-jogo).
    pub abilities: Option<Vec<AbilityState>>,
    /// `None` quando o Dota ainda não envia o inventário (antes do pré-jogo),
    /// mesmo que já haja itens comprados.
    pub inventory: Option<Vec<CarriedItem>>,
}

impl GameSnapshot {
    /// Evento sem dados de partida, como os enviados no menu.
    pub fn menu() -> Self {
        Self {
            phase: None,
            clock_time: None,
            match_id: None,
            hero: HeroPick::NotPicked,
            hero_level: None,
            talents_learned: None,
            hero_alive: None,
            team: None,
            winner: None,
            stats: PlayerStats::default(),
            abilities: None,
            inventory: None,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/game_tests.rs"]
mod tests;
