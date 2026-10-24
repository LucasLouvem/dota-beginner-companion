//! Grava os eventos GSI em um arquivo JSON Lines (um evento por linha), para
//! acompanhar com `tail -f` durante o desenvolvimento.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::application::ports::{GsiRecordError, GsiRecorder};

/// Gravador que acrescenta cada evento ao fim do arquivo informado.
pub struct JsonLinesGsiRecorder {
    path: PathBuf,
}

impl JsonLinesGsiRecorder {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl GsiRecorder for JsonLinesGsiRecorder {
    /// Cada linha tem `received_at_ms` (Unix, em milissegundos) e o `payload`
    /// completo. O arquivo é criado com permissão 0600 porque o payload traz
    /// SteamID e nome do jogador.
    fn record(&self, received_at: SystemTime, raw: &str) -> Result<(), GsiRecordError> {
        let payload: Value =
            serde_json::from_str(raw).map_err(|_| GsiRecordError::InvalidPayload)?;
        let received_at_ms = received_at
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or_default();
        let line = json!({ "received_at_ms": received_at_ms, "payload": payload });

        let write_error = |io_error: std::io::Error| GsiRecordError::Write(io_error.kind());
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.path)
            .map_err(write_error)?;
        writeln!(file, "{line}").map_err(write_error)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/gsi_log_file_tests.rs"]
mod tests;
