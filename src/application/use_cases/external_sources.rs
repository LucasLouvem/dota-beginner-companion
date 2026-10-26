//! Fontes de dados externos compartilhadas pela coleta de partidas públicas
//! e pelas recomendações.

use std::sync::Arc;

use crate::application::ports::{CacheStore, CallBudget, PublicBuildStore, StatsProvider};
use crate::domain::patch::PatchCatalog;

/// Adaptadores e dados embutidos usados nas consultas ao OpenDota.
#[derive(Clone)]
pub struct ExternalSources {
    pub provider: Arc<dyn StatsProvider>,
    /// Orçamento global; cada consulta reserva na categoria dela.
    pub budget: Arc<dyn CallBudget>,
    pub cache: Arc<dyn CacheStore>,
    pub store: Arc<dyn PublicBuildStore>,
    /// Último recurso para o patch atual.
    pub embedded_patches: Option<PatchCatalog>,
}
