//! Dados estáticos dos itens (nome de exibição e componentes da receita),
//! consultados pelo nome interno que o GSI envia.

/// Dados estáticos de um item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInfo {
    /// Nome interno, como o GSI envia (`item_satanic`).
    pub name: String,
    /// Nome de exibição da fonte, em inglês.
    pub display_name: String,
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
