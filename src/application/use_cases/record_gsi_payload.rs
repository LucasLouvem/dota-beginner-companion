//! Decide quais eventos vão para o arquivo de log GSI e registra o resultado da gravação.

use std::sync::Arc;
use std::time::SystemTime;

use crate::application::event_bus::CoreEvent;
use crate::application::ports::GsiRecordError;
use crate::application::state::GsiLogStatus;

/// Evento a gravar: apenas eventos aceitos vindos do Dota, sem os testes manuais.
pub fn payload_to_record(event: &CoreEvent) -> Option<(SystemTime, Arc<str>)> {
    match event {
        CoreEvent::GsiAccepted {
            raw,
            at,
            is_test: false,
        } => Some((*at, Arc::clone(raw))),
        _ => None,
    }
}

/// Conta a gravação bem-sucedida ou guarda o erro para exibir no Diagnóstico.
pub fn apply_record_result(status: &mut GsiLogStatus, result: Result<(), GsiRecordError>) {
    match result {
        Ok(()) => {
            status.written += 1;
            status.last_error = None;
        }
        Err(record_error) => status.last_error = Some(record_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::event_bus::RejectReason;
    use std::io;
    use std::path::PathBuf;

    fn accepted(is_test: bool) -> CoreEvent {
        CoreEvent::GsiAccepted {
            raw: "{}".into(),
            at: SystemTime::now(),
            is_test,
        }
    }

    fn empty_status() -> GsiLogStatus {
        GsiLogStatus {
            path: PathBuf::from("logs/gsi.jsonl"),
            written: 0,
            last_error: None,
        }
    }

    #[test]
    fn records_events_accepted_from_dota() {
        assert!(payload_to_record(&accepted(false)).is_some());
    }

    #[test]
    fn skips_manual_test_events() {
        assert!(payload_to_record(&accepted(true)).is_none());
    }

    #[test]
    fn skips_rejected_requests() {
        let rejected = CoreEvent::GsiRejected {
            reason: RejectReason::InvalidToken,
            at: SystemTime::now(),
        };
        assert!(payload_to_record(&rejected).is_none());
    }

    #[test]
    fn successful_write_increments_counter_and_clears_previous_error() {
        let mut status = GsiLogStatus {
            last_error: Some(GsiRecordError::InvalidPayload),
            ..empty_status()
        };
        apply_record_result(&mut status, Ok(()));
        assert_eq!(status.written, 1);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn failed_write_keeps_counter_and_stores_error() {
        let mut status = empty_status();
        let write_error = GsiRecordError::Write(io::ErrorKind::PermissionDenied);
        apply_record_result(&mut status, Err(write_error));
        assert_eq!(status.written, 0);
        assert_eq!(status.last_error, Some(write_error));
    }
}
