//! Auxiliares compartilhados pelos testes unitários: fakes dos ports e
//! captura de logs.

mod gsi_fixtures;
mod log_capture;
mod memory_files;

pub use gsi_fixtures::{
    DRAGON_KNIGHT_MATCH, DRAGON_KNIGHT_MATCH_END, ZEUS_MATCH, fixture_payloads,
};
pub use log_capture::{CapturedLogs, capture_logs};
pub use memory_files::MemoryFiles;
