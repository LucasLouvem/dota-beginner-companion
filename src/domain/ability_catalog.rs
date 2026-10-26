//! Nomes internos das habilidades pelo ID numérico, usado pelo OpenDota na
//! ordem em que as habilidades foram evoluídas (`ability_upgrades_arr`).

use std::collections::HashMap;

/// Nomes internos das habilidades, pelo ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbilityIdCatalog {
    names: HashMap<u32, String>,
}

impl AbilityIdCatalog {
    pub fn new(names: HashMap<u32, String>) -> Self {
        Self { names }
    }

    /// Nome interno da habilidade com o ID dado, como `zuus_arc_lightning`.
    pub fn name(&self, id: u32) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    /// Talento: habilidade cujo nome interno começa com `special_bonus`.
    /// Um ID desconhecido não é talento.
    pub fn is_talent(&self, id: u32) -> bool {
        self.name(id)
            .is_some_and(|name| name.starts_with("special_bonus"))
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/ability_catalog_tests.rs"]
mod tests;
