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
            ..
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
#[path = "../../../tests/unit/application/use_cases/record_gsi_payload_tests.rs"]
mod tests;
