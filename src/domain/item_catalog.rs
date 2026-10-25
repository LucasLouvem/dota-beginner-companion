//! Dados estáticos dos itens (nome de exibição, custo e componentes da
//! receita), consultados pelo nome interno que o GSI envia.

/// Custo mínimo de um item grande, o mesmo corte que o OpenDota usa para os
/// itens do meio do jogo na popularidade por fase.
pub const MAJOR_ITEM_MIN_COST: u32 = 2000;

/// Dados estáticos de um item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInfo {
    /// ID numérico da fonte, usado pelo OpenDota no inventário das partidas.
    pub id: u32,
    /// Nome interno, como o GSI envia (`item_satanic`).
    pub name: String,
    /// Nome de exibição da fonte, em inglês.
    pub display_name: String,
    /// Custo em ouro; ausente em alguns itens da fonte (como os neutros).
    pub cost: Option<u32>,
    /// Itens que formam a receita, pelo nome interno; vazio quando o item não
    /// é montado a partir de outros.
    pub components: Vec<String>,
}

/// Conjunto de itens conhecidos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemCatalog {
    items: Vec<ItemInfo>,
}

impl ItemCatalog {
    pub fn new(items: Vec<ItemInfo>) -> Self {
        Self { items }
    }

    /// Busca pelo nome interno.
    pub fn by_name(&self, name: &str) -> Option<&ItemInfo> {
        self.items.iter().find(|item| item.name == name)
    }

    /// Busca pelo ID numérico da fonte.
    pub fn by_id(&self, id: u32) -> Option<&ItemInfo> {
        self.items.iter().find(|item| item.id == id)
    }

    /// Nome de exibição; o nome interno quando o item não está no catálogo.
    pub fn display_name<'a>(&'a self, name: &'a str) -> &'a str {
        self.by_name(name)
            .map_or(name, |item| item.display_name.as_str())
    }

    /// Componentes da receita; vazio para item sem receita ou desconhecido.
    pub fn components(&self, name: &str) -> &[String] {
        self.by_name(name)
            .map_or(&[], |item| item.components.as_slice())
    }

    /// Custo em ouro, quando a fonte o informa.
    pub fn cost(&self, name: &str) -> Option<u32> {
        self.by_name(name).and_then(|item| item.cost)
    }

    /// Item grande: montado a partir de outros (tem receita) e com custo de
    /// pelo menos `MAJOR_ITEM_MIN_COST`. Item desconhecido ou sem custo não é.
    pub fn is_major_item(&self, name: &str) -> bool {
        self.by_name(name).is_some_and(|item| {
            !item.components.is_empty() && item.cost.is_some_and(|cost| cost >= MAJOR_ITEM_MIN_COST)
        })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/item_catalog_tests.rs"]
mod tests;
