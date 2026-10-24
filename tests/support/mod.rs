//! Auxiliares compartilhados pelos testes unitários: fakes dos ports e
//! captura de logs.

mod log_capture;
mod memory_files;

pub use log_capture::capture_logs;
pub use memory_files::MemoryFiles;
