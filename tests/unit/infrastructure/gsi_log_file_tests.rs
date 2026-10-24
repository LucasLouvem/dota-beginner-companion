//! Testes unitários de `src/infrastructure/gsi_log_file.rs`.

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
fn fails_with_write_error_and_path_when_directory_does_not_exist() {
    let temp_dir = tempfile::tempdir().unwrap();
    let log_path = temp_dir.path().join("missing/gsi.jsonl");
    let recorder = JsonLinesGsiRecorder::new(log_path.clone());

    let result = recorder.record(SystemTime::now(), "{}");
    let Err(GsiRecordError::Write { path, source }) = &result else {
        panic!("esperava erro de gravação: {result:?}");
    };
    assert_eq!(path, &log_path);
    assert_eq!(source.kind(), io::ErrorKind::NotFound);
}

#[test]
fn fails_with_invalid_payload_and_writes_nothing_when_text_is_not_json() {
    let temp_dir = tempfile::tempdir().unwrap();
    let log_path = temp_dir.path().join("gsi.jsonl");
    let recorder = JsonLinesGsiRecorder::new(log_path.clone());

    let result = recorder.record(SystemTime::now(), "not json");
    let Err(GsiRecordError::InvalidPayload { source }) = &result else {
        panic!("esperava payload inválido: {result:?}");
    };
    let parse_error = source
        .downcast_ref::<serde_json::Error>()
        .expect("a causa é o erro original do serde_json");
    assert_eq!(parse_error.classify(), serde_json::error::Category::Syntax);
    assert!(!log_path.exists());
}
