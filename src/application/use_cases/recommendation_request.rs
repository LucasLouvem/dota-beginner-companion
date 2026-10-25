//! Consulta de recomendações em andamento. Cada consulta recebe a chave
//! (herói, modo, patch) e uma geração crescente; só a resposta da consulta
//! atual pode ser publicada. Uma resposta atrasada de outro herói ou modo,
//! ou de uma consulta já substituída, é descartada.

use crate::domain::game::GameMode;

/// O que define as recomendações exibidas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationKey {
    /// ID do herói no Dota (o mesmo do GSI e do OpenDota).
    pub hero_id: u32,
    pub mode: GameMode,
    pub patch_id: u32,
}

/// Identifica uma consulta iniciada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestTicket {
    pub generation: u64,
    pub key: RecommendationKey,
}

/// Consulta atual e a geração da última iniciada ou cancelada.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestTracker {
    generation: u64,
    current: Option<RecommendationKey>,
}

impl RequestTracker {
    /// Inicia a consulta de `key`. Devolve `None` quando ela já é a atual,
    /// para não repetir chamadas a cada estado do jogo.
    pub fn start(&mut self, key: RecommendationKey) -> Option<RequestTicket> {
        if self.current.as_ref() == Some(&key) {
            return None;
        }
        Some(self.restart(key))
    }

    /// Inicia uma consulta nova mesmo que a chave seja a atual (por exemplo,
    /// para atualizar os dados); a anterior deixa de valer.
    pub fn restart(&mut self, key: RecommendationKey) -> RequestTicket {
        self.generation += 1;
        self.current = Some(key.clone());
        RequestTicket {
            generation: self.generation,
            key,
        }
    }

    /// Não há mais o que consultar (sem herói, por exemplo); respostas
    /// pendentes deixam de valer.
    pub fn clear(&mut self) {
        self.generation += 1;
        self.current = None;
    }

    /// A resposta de `ticket` ainda é a da consulta atual.
    pub fn accepts(&self, ticket: &RequestTicket) -> bool {
        ticket.generation == self.generation && self.current.as_ref() == Some(&ticket.key)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/recommendation_request_tests.rs"]
mod tests;
