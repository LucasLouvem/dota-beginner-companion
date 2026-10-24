//! Interfaces que os adaptadores (infraestrutura, GSI, providers) implementam.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::SystemTime;

use crate::domain::dota_install::DotaInstall;
use crate::domain::gsi_config::GsiEndpoint;

/// Future com tipo apagado, para traits assíncronas usadas como `dyn`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Bibliotecas Steam verificadas e instalações do Dota encontradas nelas.
pub struct LocatorResult {
    pub searched_libraries: Vec<PathBuf>,
    pub installs: Vec<DotaInstall>,
}

/// Localiza as instalações do Dota 2.
pub trait DotaLocator: Send + Sync {
    fn locate(&self) -> LocatorResult;
}

/// Informa se o Dota 2 está em execução.
pub trait ProcessProbe: Send + Sync {
    fn dota_running(&self) -> bool;
}

/// Acesso aos arquivos de configuração GSI. Mantém a política
/// (backup, confirmação) no caso de uso e só o I/O no adaptador.
pub trait ConfigFiles: Send + Sync {
    fn read(&self, path: &Path) -> io::Result<Option<String>>;
    fn ensure_dir(&self, dir: &Path) -> io::Result<()>;
    fn write(&self, path: &Path, contents: &str) -> io::Result<()>;
    /// Copia o arquivo para um nome de backup único e devolve o caminho dele.
    fn backup(&self, path: &Path) -> io::Result<PathBuf>;
}

/// Falha ao gravar um evento GSI no arquivo de log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GsiRecordError {
    #[error("não foi possível abrir ou gravar o arquivo ({0})")]
    Write(io::ErrorKind),
    #[error("o evento recebido não é um JSON válido")]
    InvalidPayload,
}

/// Grava os eventos GSI recebidos do Dota, em ordem de chegada.
pub trait GsiRecorder: Send + Sync {
    fn record(&self, received_at: SystemTime, raw: &str) -> Result<(), GsiRecordError>;
}

/// Verifica se o servidor GSI local aceita eventos.
pub trait ConnectionTester: Send + Sync {
    /// Envia um evento de teste ao servidor GSI local.
    fn send_test_event<'a>(
        &'a self,
        endpoint: &'a GsiEndpoint,
    ) -> BoxFuture<'a, Result<(), String>>;
}
