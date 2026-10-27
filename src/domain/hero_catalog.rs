//! Dados estáticos dos heróis (nome, atributo, tipo de ataque, funções e
//! habilidades), consultados pelo nome interno ou pelo id que o GSI envia.

/// Atributo primário do herói.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimaryAttribute {
    Strength,
    Agility,
    Intelligence,
    Universal,
    /// Valor que a fonte trouxe e o aplicativo não conhece.
    Other(String),
}

impl PrimaryAttribute {
    /// Converte a abreviação da fonte (`str`, `agi`, `int`, `all`).
    pub fn from_code(code: &str) -> Self {
        match code {
            "str" => PrimaryAttribute::Strength,
            "agi" => PrimaryAttribute::Agility,
            "int" => PrimaryAttribute::Intelligence,
            "all" => PrimaryAttribute::Universal,
            other => PrimaryAttribute::Other(other.to_owned()),
        }
    }
}

/// Tipo de ataque do herói.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttackType {
    Melee,
    Ranged,
    Other(String),
}

impl AttackType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Melee" => AttackType::Melee,
            "Ranged" => AttackType::Ranged,
            other => AttackType::Other(other.to_owned()),
        }
    }
}

/// Função que a fonte atribui ao herói. Não é a posição (1 a 5) na partida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeroRole {
    Carry,
    Support,
    Nuker,
    Disabler,
    Durable,
    Escape,
    Pusher,
    Initiator,
    Other(String),
}

impl HeroRole {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Carry" => HeroRole::Carry,
            "Support" => HeroRole::Support,
            "Nuker" => HeroRole::Nuker,
            "Disabler" => HeroRole::Disabler,
            "Durable" => HeroRole::Durable,
            "Escape" => HeroRole::Escape,
            "Pusher" => HeroRole::Pusher,
            "Initiator" => HeroRole::Initiator,
            other => HeroRole::Other(other.to_owned()),
        }
    }
}

/// Habilidade listada para o herói na fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityInfo {
    /// Nome interno, como o GSI envia (`zuus_arc_lightning`).
    pub name: String,
    /// Nome de exibição da fonte, em inglês.
    pub display_name: String,
    /// Caminho da imagem da habilidade no CDN da Steam, como na fonte.
    pub image_path: Option<String>,
}

/// Dados estáticos de um herói.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroInfo {
    pub id: u32,
    /// Nome interno, como o GSI envia (`npc_dota_hero_zuus`).
    pub name: String,
    /// Nome de exibição da fonte, em inglês.
    pub display_name: String,
    pub primary_attribute: PrimaryAttribute,
    pub attack_type: AttackType,
    pub roles: Vec<HeroRole>,
    pub abilities: Vec<AbilityInfo>,
    /// Caminho do retrato no CDN da Steam, como na fonte.
    pub image_path: Option<String>,
}

impl HeroInfo {
    /// Nome de exibição de uma habilidade do herói, se a fonte a lista.
    pub fn ability_display_name(&self, ability: &str) -> Option<&str> {
        self.abilities
            .iter()
            .find(|info| info.name == ability)
            .map(|info| info.display_name.as_str())
    }
}

/// Conjunto de heróis conhecidos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeroCatalog {
    heroes: Vec<HeroInfo>,
}

impl HeroCatalog {
    pub fn new(heroes: Vec<HeroInfo>) -> Self {
        Self { heroes }
    }

    /// Busca pelo nome interno.
    pub fn by_name(&self, name: &str) -> Option<&HeroInfo> {
        self.heroes.iter().find(|hero| hero.name == name)
    }

    /// Busca pelo id numérico.
    pub fn by_id(&self, id: u32) -> Option<&HeroInfo> {
        self.heroes.iter().find(|hero| hero.id == id)
    }

    /// Nome de exibição de uma habilidade de qualquer herói do catálogo.
    pub fn ability_display_name(&self, ability: &str) -> Option<&str> {
        self.heroes
            .iter()
            .find_map(|hero| hero.ability_display_name(ability))
    }

    /// Caminho da imagem de um herói (pelo nome interno, `npc_dota_hero_*`)
    /// ou de uma habilidade (pelo nome interno dela).
    pub fn image_path_of(&self, key: &str) -> Option<&str> {
        if key.starts_with("npc_dota_hero_") {
            return self.by_name(key)?.image_path.as_deref();
        }
        self.heroes
            .iter()
            .flat_map(|hero| &hero.abilities)
            .find(|ability| ability.name == key)?
            .image_path
            .as_deref()
    }

    pub fn len(&self) -> usize {
        self.heroes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heroes.is_empty()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/hero_catalog_tests.rs"]
mod tests;
