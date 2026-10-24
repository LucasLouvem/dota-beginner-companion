//! Captura os logs do `tracing` emitidos na thread do teste.
//!
//! O `tracing` guarda em cache global se cada ponto de log está ativo. Com
//! `set_default` por teste, outro teste rodando em paralelo sem subscriber
//! pode deixar esse cache desligado e o log não chega. Por isso há um único
//! subscriber global, instalado uma vez, que escreve no buffer da thread que
//! está capturando; nas outras threads o log é descartado.

use std::cell::RefCell;
use std::io;
use std::sync::{Arc, Mutex, Once};

use tracing_subscriber::fmt::MakeWriter;

thread_local! {
    static ACTIVE_CAPTURE: RefCell<Option<CapturedLogs>> = const { RefCell::new(None) };
}

static INSTALL_SUBSCRIBER: Once = Once::new();

/// Saída acumulada dos logs, para conferir o que foi registrado.
#[derive(Clone, Default)]
pub struct CapturedLogs {
    output: Arc<Mutex<Vec<u8>>>,
}

impl CapturedLogs {
    /// Texto de todos os logs registrados até agora.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
    }
}

/// Para de capturar na thread atual quando é descartado.
pub struct CaptureGuard;

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        ACTIVE_CAPTURE.with(|active| *active.borrow_mut() = None);
    }
}

/// Escreve no buffer da thread atual, se ela estiver capturando.
struct ThreadLogWriter(Option<CapturedLogs>);

impl io::Write for ThreadLogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(logs) = &self.0 {
            logs.output.lock().unwrap().extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ThreadLogWriters;

impl<'a> MakeWriter<'a> for ThreadLogWriters {
    type Writer = ThreadLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ThreadLogWriter(ACTIVE_CAPTURE.with(|active| active.borrow().clone()))
    }
}

/// Passa a capturar os logs de todos os níveis na thread atual enquanto o
/// guard existir. Com `#[tokio::test]` (runtime de uma thread) inclui as
/// tarefas assíncronas; `spawn_blocking` roda em outra thread e fica de fora.
pub fn capture_logs() -> (CaptureGuard, CapturedLogs) {
    INSTALL_SUBSCRIBER.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThreadLogWriters)
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("só os testes instalam um subscriber global, e uma única vez");
    });
    let logs = CapturedLogs::default();
    ACTIVE_CAPTURE.with(|active| *active.borrow_mut() = Some(logs.clone()));
    (CaptureGuard, logs)
}
