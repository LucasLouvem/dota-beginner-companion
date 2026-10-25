//! Interfaces que os adaptadores (infraestrutura, GSI e storage) implementam.

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::domain::dota_install::DotaInstall;
use crate::domain::gsi_config::GsiEndpoint;
use crate::domain::match_data::{HeroMatch, MatchDetails};
use crate::domain::patch::PatchCatalog;

use super::error::{Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText, technical_details};

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
#[derive(Debug, Clone, thiserror::Error)]
pub enum GsiRecordError {
    #[error("não foi possível abrir ou gravar {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("o evento recebido não é um JSON válido")]
    InvalidPayload {
        #[source]
        source: SharedError,
    },
}

impl ToDiagnostic for GsiRecordError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Falha ao gravar o log de eventos GSI");
        let (code, cause, resolution) = match self {
            GsiRecordError::Write { path, source } => {
                let path = path.display().to_string();
                match source.kind() {
                    io::ErrorKind::NotFound => (
                        DiagnosticCode::GsiLogDirectoryMissing,
                        UiText::with("A pasta de {} não existe.", [path]),
                        "Crie a pasta ou altere DOTA_COMPANION_GSI_LOG; o aplicativo continua \
                         funcionando.",
                    ),
                    io::ErrorKind::PermissionDenied => (
                        DiagnosticCode::GsiLogPermissionDenied,
                        UiText::with("Sem permissão para gravar {}.", [path]),
                        "Corrija a permissão do arquivo ou da pasta; o aplicativo continua \
                         funcionando.",
                    ),
                    _ => {
                        return Diagnostic::unclassified(
                            title,
                            format!("Falha ao gravar {path}."),
                            self,
                            redactor,
                        );
                    }
                }
            }
            GsiRecordError::InvalidPayload { .. } => (
                DiagnosticCode::GsiLogInvalidPayload,
                UiText::new("O evento recebido não é um JSON válido e não foi gravado."),
                "Nenhuma ação é necessária; os próximos eventos válidos continuam sendo gravados.",
            ),
        };
        Diagnostic {
            code,
            title,
            cause,
            resolution: UiText::new(resolution),
            technical_details: Some(technical_details(self, redactor)),
        }
    }
}

/// Grava os eventos GSI recebidos do Dota, em ordem de chegada.
pub trait GsiRecorder: Send + Sync {
    fn record(&self, received_at: SystemTime, raw: &str) -> Result<(), GsiRecordError>;
}

/// Erro de origem de um adaptador, guardado sem expor a crate que o gerou.
pub type SharedError = Arc<dyn Error + Send + Sync>;

/// Motivo de o teste de conexão com o servidor GSI local ter falhado.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConnectionTestError {
    #[error("o servidor GSI local não aceitou a conexão")]
    Unreachable {
        #[source]
        source: SharedError,
    },
    #[error("o servidor GSI local não respondeu a tempo")]
    TimedOut {
        #[source]
        source: SharedError,
    },
    #[error("o servidor GSI recusou o token do evento de teste")]
    TokenRejected,
    #[error("o servidor GSI recusou o conteúdo do evento de teste")]
    PayloadRejected,
    #[error("resposta inesperada do servidor GSI ({details})")]
    UnexpectedResponse { details: String },
    #[error("não foi possível enviar o evento de teste")]
    Request {
        #[source]
        source: SharedError,
    },
}

impl ToDiagnostic for ConnectionTestError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Teste de conexão falhou");
        let (code, cause, resolution) = match self {
            ConnectionTestError::Unreachable { .. } => (
                DiagnosticCode::ConnectionTestUnreachable,
                "O servidor GSI local não aceitou a conexão.",
                "Verifique no Diagnóstico se o servidor local está ouvindo.",
            ),
            ConnectionTestError::TimedOut { .. } => (
                DiagnosticCode::ConnectionTestTimedOut,
                "O servidor GSI local não respondeu a tempo.",
                "Verifique no Diagnóstico se o servidor local está ouvindo e tente de novo.",
            ),
            ConnectionTestError::TokenRejected => (
                DiagnosticCode::ConnectionTestTokenRejected,
                "O servidor na porta configurada recusou o token do aplicativo.",
                "Verifique se a porta gsi_port não está sendo usada por outro programa.",
            ),
            ConnectionTestError::PayloadRejected => (
                DiagnosticCode::ConnectionTestPayloadRejected,
                "O servidor na porta configurada recusou o evento de teste.",
                "Verifique se a porta gsi_port não está sendo usada por outro programa.",
            ),
            ConnectionTestError::UnexpectedResponse { .. } => {
                return Diagnostic::unclassified(
                    title,
                    "O servidor GSI respondeu de forma inesperada.",
                    self,
                    redactor,
                );
            }
            ConnectionTestError::Request { .. } => {
                return Diagnostic::unclassified(
                    title,
                    "O evento de teste não pôde ser enviado.",
                    self,
                    redactor,
                );
            }
        };
        Diagnostic {
            code,
            title,
            cause: UiText::new(cause),
            resolution: UiText::new(resolution),
            technical_details: Some(technical_details(self, redactor)),
        }
    }
}

/// Se um valor do cache ainda vale ou já passou do prazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    /// Passou do prazo; ainda pode ser exibido, marcado como antigo.
    Stale,
}

impl Freshness {
    /// Um valor gravado em `stored_at` vale por `ttl` a partir dali. Um horário
    /// de gravação no futuro (relógio ajustado para trás) conta como válido.
    pub fn of(stored_at: SystemTime, now: SystemTime, ttl: Duration) -> Self {
        match now.duration_since(stored_at) {
            Ok(age) if age > ttl => Freshness::Stale,
            _ => Freshness::Fresh,
        }
    }
}

/// Valor lido do cache, com o horário da gravação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedValue {
    pub value: String,
    pub stored_at: SystemTime,
    pub freshness: Freshness,
}

/// Como uma falha do banco local foi classificada pelo adaptador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageFailureKind {
    /// O arquivo não pôde ser aberto ou gravado (permissão, somente leitura).
    Unavailable,
    /// O arquivo está corrompido ou não é um banco SQLite.
    Corrupted,
    /// Causa ainda não classificada.
    Other,
}

/// Falha do banco local.
#[derive(Debug, Clone, thiserror::Error)]
pub enum StorageError {
    #[error("não foi possível abrir o banco local {}", path.display())]
    Open {
        path: PathBuf,
        kind: StorageFailureKind,
        #[source]
        source: SharedError,
    },
    #[error("a operação \"{operation}\" no banco local falhou")]
    Operation {
        operation: &'static str,
        kind: StorageFailureKind,
        #[source]
        source: SharedError,
    },
}

impl StorageError {
    pub fn kind(&self) -> StorageFailureKind {
        match self {
            StorageError::Open { kind, .. } | StorageError::Operation { kind, .. } => *kind,
        }
    }
}

impl ToDiagnostic for StorageError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let title = UiText::new("Banco local indisponível");
        let (code, cause, resolution) = match self.kind() {
            StorageFailureKind::Unavailable => (
                DiagnosticCode::StorageUnavailable,
                "O banco local não pôde ser aberto ou gravado.",
                "Verifique a permissão da pasta de dados do aplicativo.",
            ),
            StorageFailureKind::Corrupted => (
                DiagnosticCode::StorageCorrupted,
                "O banco local está corrompido ou não é um banco SQLite.",
                "Apague o arquivo do banco local; o aplicativo cria um novo e o cache é refeito.",
            ),
            StorageFailureKind::Other => {
                return Diagnostic::unclassified(title, "O banco local falhou.", self, redactor);
            }
        };
        Diagnostic {
            code,
            title,
            cause: UiText::new(cause),
            resolution: UiText::new(resolution),
            technical_details: Some(technical_details(self, redactor)),
        }
    }
}

/// Cache local de dados baixados, com prazo de validade (TTL).
pub trait CacheStore: Send + Sync {
    /// Grava ou substitui o valor da chave.
    fn put(&self, key: &str, value: &str, stored_at: SystemTime) -> Result<(), StorageError>;
    /// Valor da chave, marcado como antigo se passou de `ttl`; `None` se não existe.
    fn get(
        &self,
        key: &str,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<Option<CachedValue>, StorageError>;
    /// Remove os valores gravados antes de `cutoff` e devolve quantos saíram.
    fn purge_older_than(&self, cutoff: SystemTime) -> Result<usize, StorageError>;
}

/// Verifica se o servidor GSI local aceita eventos.
pub trait ConnectionTester: Send + Sync {
    /// Envia um evento de teste ao servidor GSI local.
    fn send_test_event<'a>(
        &'a self,
        endpoint: &'a GsiEndpoint,
    ) -> BoxFuture<'a, Result<(), ConnectionTestError>>;
}

/// Para que serve uma chamada ao OpenDota. Cada categoria tem seus limites no
/// orçamento do aplicativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallCategory {
    /// Consulta pedida por uma tela; tem prioridade.
    OnDemand,
    /// Coleta de partidas públicas em segundo plano.
    Collection,
}

impl CallCategory {
    /// Nome gravado no banco local.
    pub const fn as_str(self) -> &'static str {
        match self {
            CallCategory::OnDemand => "on_demand",
            CallCategory::Collection => "collection",
        }
    }
}

/// Limite do orçamento que foi atingido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetLimit {
    DailyTotal,
    DailyCollection,
    MonthlyTotal,
    MonthlyCollection,
    /// Limite de chamadas de uma única consulta (lista, detalhes e novas
    /// tentativas).
    PerQuery,
}

impl fmt::Display for BudgetLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            BudgetLimit::DailyTotal => "total do dia",
            BudgetLimit::DailyCollection => "da coleta no dia",
            BudgetLimit::MonthlyTotal => "total do mês",
            BudgetLimit::MonthlyCollection => "da coleta no mês",
            BudgetLimit::PerQuery => "da consulta",
        })
    }
}

/// O orçamento de chamadas do aplicativo acabou para a categoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("o limite {limit} de chamadas ao OpenDota foi atingido")]
pub struct BudgetExhausted {
    pub category: CallCategory,
    pub limit: BudgetLimit,
}

/// Falha ao reservar uma chamada ao OpenDota.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BudgetError {
    #[error(transparent)]
    Exhausted(#[from] BudgetExhausted),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl ToDiagnostic for BudgetError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        match self {
            BudgetError::Exhausted(_) => Diagnostic {
                code: DiagnosticCode::OpendotaBudgetExhausted,
                title: UiText::new("Limite de consultas ao OpenDota atingido"),
                cause: UiText::new(
                    "O aplicativo limita as próprias chamadas ao OpenDota para não sobrecarregar o serviço.",
                ),
                resolution: UiText::new(
                    "Os dados guardados continuam aparecendo. Novas consultas voltam quando o limite renovar (o dia e o mês contam em UTC).",
                ),
                technical_details: Some(technical_details(self, redactor)),
            },
            BudgetError::Storage(storage_error) => storage_error.to_diagnostic(redactor),
        }
    }
}

/// Orçamento de chamadas ao OpenDota, persistido. Cada chamada (inclusive
/// novas tentativas e respostas 429) reserva uma unidade antes de sair.
pub trait CallBudget: Send + Sync {
    /// Reserva uma chamada da categoria no período de `now`, ou informa qual
    /// limite acabou. Conferir e reservar é uma única operação.
    fn reserve(&self, category: CallCategory, now: SystemTime) -> Result<(), BudgetError>;
}

/// Falha ao consultar o OpenDota. `path` é o caminho da consulta, sem o
/// endereço do serviço.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("não foi possível conectar ao OpenDota ({path})")]
    Unreachable {
        path: String,
        #[source]
        source: SharedError,
    },
    #[error("o OpenDota não respondeu a tempo ({path})")]
    TimedOut {
        path: String,
        #[source]
        source: SharedError,
    },
    #[error("o OpenDota recusou por excesso de chamadas, HTTP 429 ({path})")]
    RateLimited { path: String },
    #[error("o OpenDota respondeu com HTTP {status} ({path})")]
    UnexpectedStatus { path: String, status: u16 },
    #[error("a resposta do OpenDota passou de {limit} bytes ({path})")]
    BodyTooLarge { path: String, limit: usize },
    #[error("a resposta do OpenDota não tem o formato esperado: {detail} ({path})")]
    UnexpectedFormat { path: String, detail: &'static str },
    #[error("a resposta do OpenDota não é um JSON válido ({path})")]
    InvalidJson {
        path: String,
        #[source]
        source: SharedError,
    },
    #[error(transparent)]
    Budget(#[from] BudgetError),
}

impl ToDiagnostic for ProviderError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let (code, cause, resolution) = match self {
            ProviderError::Budget(budget_error) => return budget_error.to_diagnostic(redactor),
            ProviderError::Unreachable { .. } => (
                DiagnosticCode::OpendotaUnreachable,
                "Não foi possível conectar ao OpenDota.",
                "Verifique a conexão com a internet. Os dados guardados continuam aparecendo.",
            ),
            ProviderError::TimedOut { .. } => (
                DiagnosticCode::OpendotaTimedOut,
                "O OpenDota não respondeu a tempo.",
                "Tente de novo mais tarde. Os dados guardados continuam aparecendo.",
            ),
            ProviderError::RateLimited { .. } => (
                DiagnosticCode::OpendotaRateLimited,
                "O OpenDota recusou a consulta por excesso de chamadas.",
                "Espere alguns minutos; o aplicativo tenta de novo nas próximas consultas.",
            ),
            ProviderError::UnexpectedStatus { .. }
            | ProviderError::UnexpectedFormat { .. }
            | ProviderError::BodyTooLarge { .. }
            | ProviderError::InvalidJson { .. } => (
                DiagnosticCode::OpendotaBadResponse,
                "O OpenDota respondeu de um jeito inesperado.",
                "Tente de novo mais tarde; se continuar, o formato da API pode ter mudado.",
            ),
        };
        Diagnostic {
            code,
            title: UiText::new("Dados do OpenDota indisponíveis"),
            cause: UiText::new(cause),
            resolution: UiText::new(resolution),
            technical_details: Some(technical_details(self, redactor)),
        }
    }
}

/// Consulta a uma fonte de estatísticas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsQuery {
    /// Lista de patches do jogo.
    Patches,
    /// Partidas recentes com o herói.
    HeroMatches { hero_id: u32 },
    /// Detalhes de uma partida.
    MatchDetails { match_id: u64 },
}

impl StatsQuery {
    /// Chave da resposta no cache local.
    pub fn cache_key(&self) -> String {
        match self {
            StatsQuery::Patches => "stats/patches".into(),
            StatsQuery::HeroMatches { hero_id } => format!("stats/heroes/{hero_id}/matches"),
            StatsQuery::MatchDetails { match_id } => format!("stats/matches/{match_id}"),
        }
    }
}

/// Fonte de estatísticas do jogo (o OpenDota). `fetch` traz a resposta como
/// texto JSON, que vai para o cache; os `decode_*` a convertem, para a
/// aplicação não depender do formato da fonte. Cada tentativa de `fetch`
/// reserva uma chamada em `budget`.
pub trait StatsProvider: Send + Sync {
    fn fetch<'a>(
        &'a self,
        query: &'a StatsQuery,
        category: CallCategory,
        budget: &'a dyn CallBudget,
    ) -> BoxFuture<'a, Result<String, ProviderError>>;

    /// Patches a partir da resposta de `StatsQuery::Patches`.
    fn decode_patches(&self, raw: &str) -> Result<PatchCatalog, ProviderError>;

    /// Partidas a partir da resposta de `StatsQuery::HeroMatches`.
    fn decode_hero_matches(&self, raw: &str) -> Result<Vec<HeroMatch>, ProviderError>;

    /// Detalhes a partir da resposta de `StatsQuery::MatchDetails`.
    fn decode_match_details(&self, raw: &str) -> Result<MatchDetails, ProviderError>;
}

#[cfg(test)]
#[path = "../../tests/unit/application/ports_tests.rs"]
mod tests;
