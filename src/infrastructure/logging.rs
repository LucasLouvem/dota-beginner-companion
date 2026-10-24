//! Filtro de log a partir do `RUST_LOG`, com aviso quando há diretiva
//! inválida em vez de ignorá-la em silêncio.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::{LevelFilter, ParseError};

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};

/// `RUST_LOG` com uma diretiva que o `tracing` não entende.
#[derive(Debug, thiserror::Error)]
#[error("o RUST_LOG tem uma diretiva inválida")]
pub struct LogFilterError(#[source] ParseError);

impl ToDiagnostic for LogFilterError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        Diagnostic::unclassified(
            UiText::new("Nível de log inválido"),
            "O log mostra só erros até o RUST_LOG ser corrigido.",
            self,
            redactor,
        )
    }
}

/// Filtro com as diretivas do `RUST_LOG`. Sem diretivas, ou com alguma
/// inválida, mostra só erros, como o `EnvFilter::from_default_env`.
pub fn log_filter(directives: Option<&str>) -> (EnvFilter, Option<LogFilterError>) {
    let builder = EnvFilter::builder().with_default_directive(LevelFilter::ERROR.into());
    match builder.parse(directives.unwrap_or_default()) {
        Ok(filter) => (filter, None),
        Err(parse_error) => (builder.parse_lossy(""), Some(LogFilterError(parse_error))),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/infrastructure/logging_tests.rs"]
mod tests;
