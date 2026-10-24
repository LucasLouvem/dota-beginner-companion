//! Filtro de log a partir do `RUST_LOG`, com aviso quando há diretiva
//! inválida em vez de ignorá-la em silêncio.

use std::io::{self, Write};
use std::path::Path;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::{LevelFilter, ParseError};

use crate::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};

/// Instala a saída de pânicos antes de criar threads. Não imprime o conteúdo
/// do pânico, nomes de threads ou backtrace, que podem revelar dados pessoais.
/// A propagação do pânico permanece sob controle do runtime.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let mut stderr = io::stderr().lock();
        let result = match info.location() {
            Some(location) => {
                let file = Path::new(location.file())
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                writeln!(
                    stderr,
                    "Falha interna (pânico) em {file}:{}:{}.",
                    location.line(),
                    location.column()
                )
            }
            None => writeln!(stderr, "Falha interna (pânico); localização indisponível."),
        };
        // Se o próprio terminal falhar, não há saída alternativa segura aqui.
        // O hook não pode provocar outro pânico nem imprimir o payload original.
        let _ = result;
    }));
}

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
