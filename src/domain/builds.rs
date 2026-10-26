//! Build real de um jogador numa partida, como a fonte de estatísticas (o
//! OpenDota) informa. O aplicativo mostra a build como o jogador fez: não
//! agrupa partidas nem escolhe ou esconde itens.

/// Registro de compra de uma partida analisada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purchase {
    /// Nome interno, como o GSI usa (`item_black_king_bar`).
    pub item: String,
    /// Segundos de partida; negativo antes do início.
    pub time: i32,
}

/// Build real de um jogador numa partida, como a fonte informa: o
/// inventário no fim da partida e, em partida analisada, todas as compras.
/// Nenhum item é filtrado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerBuild {
    /// Itens de `item_0..5`, na ordem dos espaços, sem os espaços vazios.
    pub inventory: Vec<String>,
    /// Itens da mochila (`backpack_0..2`), sem os espaços vazios.
    pub backpack: Vec<String>,
    pub neutral: Option<String>,
    /// Aprimoramento do item neutro (`item_neutral2`).
    pub neutral_enhancement: Option<String>,
    /// Todas as compras, em ordem de horário (ver `in_purchase_order`);
    /// ausente em partida não analisada.
    pub purchases: Option<Vec<Purchase>>,
}

/// Compras em ordem de horário. A ordenação é estável: compras no mesmo
/// horário mantêm a ordem da fonte.
pub fn in_purchase_order(mut purchases: Vec<Purchase>) -> Vec<Purchase> {
    purchases.sort_by_key(|purchase| purchase.time);
    purchases
}

#[cfg(test)]
#[path = "../../tests/unit/domain/builds_tests.rs"]
mod tests;
