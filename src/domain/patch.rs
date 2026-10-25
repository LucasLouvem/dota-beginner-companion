//! Patches do Dota, pelo ID que o OpenDota usa no campo `patch` das partidas.

/// Um patch, como `7.41`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub id: u32,
    /// Versão, como `7.41`.
    pub name: String,
}

/// Patches conhecidos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatchCatalog {
    patches: Vec<Patch>,
}

impl PatchCatalog {
    pub fn new(patches: Vec<Patch>) -> Self {
        Self { patches }
    }

    /// O patch mais recente: o de maior ID, porque a fonte numera em ordem
    /// de lançamento.
    pub fn current(&self) -> Option<&Patch> {
        self.patches.iter().max_by_key(|patch| patch.id)
    }

    pub fn by_id(&self, id: u32) -> Option<&Patch> {
        self.patches.iter().find(|patch| patch.id == id)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/patch_tests.rs"]
mod tests;
