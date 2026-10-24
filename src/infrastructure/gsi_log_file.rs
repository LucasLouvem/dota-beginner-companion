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
mod tests {
    use super::*;
    use std::fs;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn appends_one_json_line_per_event_in_arrival_order() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_path = temp_dir.path().join("gsi.jsonl");
        let recorder = JsonLinesGsiRecorder::new(log_path.clone());
        let received_at = UNIX_EPOCH + Duration::from_millis(1_500);

        recorder
            .record(received_at, "{\n  \"map\": {\"game_state\": \"A\"}\n}")
            .unwrap();
        recorder
            .record(received_at, r#"{"map":{"game_state":"B"}}"#)
            .unwrap();

        let contents = fs::read_to_string(&log_path).unwrap();
        let lines: Vec<Value> = contents
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["received_at_ms"], 1_500);
        assert_eq!(lines[0]["payload"]["map"]["game_state"], "A");
        assert_eq!(lines[1]["payload"]["map"]["game_state"], "B");
    }

    #[test]
    fn creates_log_file_readable_only_by_owner() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_path = temp_dir.path().join("gsi.jsonl");
        JsonLinesGsiRecorder::new(log_path.clone())
            .record(SystemTime::now(), "{}")
            .unwrap();
        assert_eq!(
            fs::metadata(&log_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn fails_with_write_error_when_directory_does_not_exist() {
        let temp_dir = tempfile::tempdir().unwrap();
        let recorder = JsonLinesGsiRecorder::new(temp_dir.path().join("missing/gsi.jsonl"));
        assert_eq!(
            recorder.record(SystemTime::now(), "{}"),
            Err(GsiRecordError::Write(io::ErrorKind::NotFound))
        );
    }

    #[test]
    fn fails_with_invalid_payload_when_text_is_not_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let recorder = JsonLinesGsiRecorder::new(temp_dir.path().join("gsi.jsonl"));
        assert_eq!(
            recorder.record(SystemTime::now(), "not json"),
            Err(GsiRecordError::InvalidPayload)
        );
    }
}
