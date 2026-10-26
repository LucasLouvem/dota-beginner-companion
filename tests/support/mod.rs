//! Auxiliares compartilhados pelos testes unitários: fakes dos ports e
//! captura de logs.

mod fake_stats;
mod gsi_fixtures;
mod log_capture;
mod memory_cache;
mod memory_files;
mod memory_public_builds;
mod recommendation_data;
mod unlimited_budget;

pub use fake_stats::{FakeResponse, FakeStatsProvider};
pub use gsi_fixtures::{
    DRAGON_KNIGHT_MATCH, DRAGON_KNIGHT_MATCH_END, ZEUS_MATCH, fixture_payloads,
};
pub use log_capture::{CapturedLogs, capture_logs};
pub use memory_cache::MemoryCache;
pub use memory_files::MemoryFiles;
pub use memory_public_builds::MemoryPublicBuildStore;
pub use recommendation_data::{pro_builds, recommendations};
pub use unlimited_budget::UnlimitedBudget;
