//! Fonte de estatísticas em memória, para os casos de uso que consultam o
//! OpenDota.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::application::ports::{
    BoxFuture, CallBudget, CallCategory, ProviderError, StatsProvider, StatsQuery,
};
use crate::domain::match_data::{HeroMatch, ItemPopularity, MatchDetails, PublicMatchSummary};
use crate::domain::patch::PatchCatalog;

/// Resposta registrada para uma consulta.
#[derive(Debug, Clone)]
pub enum FakeResponse {
    Patches(PatchCatalog),
    HeroMatches(Vec<HeroMatch>),
    MatchDetails(MatchDetails),
    PublicMatches(Vec<PublicMatchSummary>),
    ItemPopularity(ItemPopularity),
    /// A busca funciona, mas a resposta não decodifica (formato novo).
    Malformed,
}

/// `fetch` reserva uma chamada no orçamento recebido, como o cliente real,
/// e devolve a chave de cache da consulta como se fosse o texto JSON; os
/// `decode_*` buscam a resposta registrada para essa chave. Consulta sem
/// resposta registrada falha como rede indisponível.
#[derive(Default)]
pub struct FakeStatsProvider {
    responses: Mutex<HashMap<String, FakeResponse>>,
    fetches: AtomicU32,
}

impl FakeStatsProvider {
    pub fn respond(&self, query: &StatsQuery, response: FakeResponse) {
        self.responses
            .lock()
            .unwrap()
            .insert(query.cache_key(), response);
    }

    /// Buscas que chegaram a reservar orçamento.
    pub fn fetches(&self) -> u32 {
        self.fetches.load(Ordering::SeqCst)
    }

    fn response(&self, raw: &str) -> Option<FakeResponse> {
        self.responses.lock().unwrap().get(raw).cloned()
    }
}

fn unexpected() -> ProviderError {
    ProviderError::UnexpectedFormat {
        path: "fake".into(),
        detail: "resposta registrada de outro tipo",
    }
}

impl StatsProvider for FakeStatsProvider {
    fn fetch<'a>(
        &'a self,
        query: &'a StatsQuery,
        category: CallCategory,
        budget: &'a dyn CallBudget,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            budget.reserve(category, SystemTime::now())?;
            self.fetches.fetch_add(1, Ordering::SeqCst);
            let key = query.cache_key();
            if self.response(&key).is_some() {
                Ok(key)
            } else {
                Err(ProviderError::Unreachable {
                    path: key,
                    source: Arc::new(io::Error::other("sem resposta registrada")),
                })
            }
        })
    }

    fn decode_patches(&self, raw: &str) -> Result<PatchCatalog, ProviderError> {
        match self.response(raw) {
            Some(FakeResponse::Patches(catalog)) => Ok(catalog),
            _ => Err(unexpected()),
        }
    }

    fn decode_hero_matches(&self, raw: &str) -> Result<Vec<HeroMatch>, ProviderError> {
        match self.response(raw) {
            Some(FakeResponse::HeroMatches(matches)) => Ok(matches),
            _ => Err(unexpected()),
        }
    }

    fn decode_match_details(&self, raw: &str) -> Result<MatchDetails, ProviderError> {
        match self.response(raw) {
            Some(FakeResponse::MatchDetails(details)) => Ok(details),
            _ => Err(unexpected()),
        }
    }

    fn decode_item_popularity(&self, raw: &str) -> Result<ItemPopularity, ProviderError> {
        match self.response(raw) {
            Some(FakeResponse::ItemPopularity(popularity)) => Ok(popularity),
            _ => Err(unexpected()),
        }
    }

    fn decode_public_matches(&self, raw: &str) -> Result<Vec<PublicMatchSummary>, ProviderError> {
        match self.response(raw) {
            Some(FakeResponse::PublicMatches(summaries)) => Ok(summaries),
            _ => Err(unexpected()),
        }
    }
}
