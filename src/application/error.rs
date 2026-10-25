//! Contrato único de diagnóstico. Todo erro exibido ao usuário é convertido
//! em um `Diagnostic` pela trait `ToDiagnostic`; os erros específicos de cada
//! camada continuam tipados e guardam a causa original.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Identificador estável de uma classificação de falha. Não depende do texto
/// exibido, do idioma nem de valores variáveis como caminhos e portas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// A porta do servidor GSI já está em uso.
    GsiBindFailed,
    /// Sem permissão para escutar na porta do servidor GSI.
    GsiBindPermissionDenied,
    /// Sem permissão para ler o arquivo `.cfg` do GSI.
    GsiConfigReadPermissionDenied,
    /// O arquivo `.cfg` do GSI não está no formato KeyValues esperado.
    GsiConfigInvalid,
    /// Sem permissão para criar, gravar ou fazer backup do `.cfg` do GSI.
    GsiConfigInstallPermissionDenied,
    /// O teste de conexão não conseguiu se conectar ao servidor local.
    ConnectionTestUnreachable,
    /// O servidor local não respondeu ao teste de conexão a tempo.
    ConnectionTestTimedOut,
    /// O servidor na porta configurada recusou o token do teste de conexão.
    ConnectionTestTokenRejected,
    /// O servidor na porta configurada recusou o conteúdo do teste de conexão.
    ConnectionTestPayloadRejected,
    /// A pasta do arquivo de log de eventos GSI não existe.
    GsiLogDirectoryMissing,
    /// Sem permissão para gravar o arquivo de log de eventos GSI.
    GsiLogPermissionDenied,
    /// O evento a gravar no log não é um JSON válido.
    GsiLogInvalidPayload,
    /// O `config.toml` tem conteúdo inválido.
    AppConfigInvalid,
    /// Sem permissão para ler ou gravar o `config.toml`.
    AppConfigPermissionDenied,
    /// O `.env` do diretório de execução tem uma linha com sintaxe inválida.
    EnvFileInvalid,
    /// Sem permissão para ler o `.env` do diretório de execução.
    EnvFilePermissionDenied,
    /// Uma tarefa do núcleo terminou sem resultado (pânico, cancelamento ou
    /// canal fechado).
    CoreTaskInterrupted,
    /// A interface perdeu a comunicação com o núcleo.
    CoreUnavailable,
    /// As traduções não puderam ser preparadas; os textos ficam em PT-BR.
    TranslationsUnavailable,
    /// O banco local não pôde ser aberto ou gravado.
    StorageUnavailable,
    /// O banco local está corrompido ou não é um banco SQLite.
    StorageCorrupted,
    /// O orçamento de chamadas ao OpenDota do aplicativo acabou no período.
    OpendotaBudgetExhausted,
    /// Não foi possível conectar ao OpenDota (rede ou DNS).
    OpendotaUnreachable,
    /// O OpenDota não respondeu a tempo.
    OpendotaTimedOut,
    /// O OpenDota recusou por excesso de chamadas (HTTP 429).
    OpendotaRateLimited,
    /// O OpenDota respondeu com status inesperado, corpo grande demais ou JSON inválido.
    OpendotaBadResponse,
    /// Causa ainda não classificada; o diagnóstico traz os detalhes técnicos.
    Unclassified,
}

impl DiagnosticCode {
    /// Todos os códigos, para verificações que percorrem o catálogo.
    pub const ALL: &[DiagnosticCode] = &[
        DiagnosticCode::GsiBindFailed,
        DiagnosticCode::GsiBindPermissionDenied,
        DiagnosticCode::GsiConfigReadPermissionDenied,
        DiagnosticCode::GsiConfigInvalid,
        DiagnosticCode::GsiConfigInstallPermissionDenied,
        DiagnosticCode::ConnectionTestUnreachable,
        DiagnosticCode::ConnectionTestTimedOut,
        DiagnosticCode::ConnectionTestTokenRejected,
        DiagnosticCode::ConnectionTestPayloadRejected,
        DiagnosticCode::GsiLogDirectoryMissing,
        DiagnosticCode::GsiLogPermissionDenied,
        DiagnosticCode::GsiLogInvalidPayload,
        DiagnosticCode::AppConfigInvalid,
        DiagnosticCode::AppConfigPermissionDenied,
        DiagnosticCode::EnvFileInvalid,
        DiagnosticCode::EnvFilePermissionDenied,
        DiagnosticCode::CoreTaskInterrupted,
        DiagnosticCode::CoreUnavailable,
        DiagnosticCode::TranslationsUnavailable,
        DiagnosticCode::StorageUnavailable,
        DiagnosticCode::StorageCorrupted,
        DiagnosticCode::OpendotaBudgetExhausted,
        DiagnosticCode::OpendotaUnreachable,
        DiagnosticCode::OpendotaTimedOut,
        DiagnosticCode::OpendotaRateLimited,
        DiagnosticCode::OpendotaBadResponse,
        DiagnosticCode::Unclassified,
    ];

    /// Código em `SCREAMING_SNAKE_CASE`, como aparece na interface e no ERRORS.md.
    pub const fn as_str(self) -> &'static str {
        match self {
            DiagnosticCode::GsiBindFailed => "GSI_BIND_FAILED",
            DiagnosticCode::GsiBindPermissionDenied => "GSI_BIND_PERMISSION_DENIED",
            DiagnosticCode::GsiConfigReadPermissionDenied => "GSI_CONFIG_READ_PERMISSION_DENIED",
            DiagnosticCode::GsiConfigInvalid => "GSI_CONFIG_INVALID",
            DiagnosticCode::GsiConfigInstallPermissionDenied => {
                "GSI_CONFIG_INSTALL_PERMISSION_DENIED"
            }
            DiagnosticCode::ConnectionTestUnreachable => "CONNECTION_TEST_UNREACHABLE",
            DiagnosticCode::ConnectionTestTimedOut => "CONNECTION_TEST_TIMED_OUT",
            DiagnosticCode::ConnectionTestTokenRejected => "CONNECTION_TEST_TOKEN_REJECTED",
            DiagnosticCode::ConnectionTestPayloadRejected => "CONNECTION_TEST_PAYLOAD_REJECTED",
            DiagnosticCode::GsiLogDirectoryMissing => "GSI_LOG_DIRECTORY_MISSING",
            DiagnosticCode::GsiLogPermissionDenied => "GSI_LOG_PERMISSION_DENIED",
            DiagnosticCode::GsiLogInvalidPayload => "GSI_LOG_INVALID_PAYLOAD",
            DiagnosticCode::AppConfigInvalid => "APP_CONFIG_INVALID",
            DiagnosticCode::AppConfigPermissionDenied => "APP_CONFIG_PERMISSION_DENIED",
            DiagnosticCode::EnvFileInvalid => "ENV_FILE_INVALID",
            DiagnosticCode::EnvFilePermissionDenied => "ENV_FILE_PERMISSION_DENIED",
            DiagnosticCode::CoreTaskInterrupted => "CORE_TASK_INTERRUPTED",
            DiagnosticCode::CoreUnavailable => "CORE_UNAVAILABLE",
            DiagnosticCode::TranslationsUnavailable => "TRANSLATIONS_UNAVAILABLE",
            DiagnosticCode::StorageUnavailable => "STORAGE_UNAVAILABLE",
            DiagnosticCode::StorageCorrupted => "STORAGE_CORRUPTED",
            DiagnosticCode::OpendotaBudgetExhausted => "OPENDOTA_BUDGET_EXHAUSTED",
            DiagnosticCode::OpendotaUnreachable => "OPENDOTA_UNREACHABLE",
            DiagnosticCode::OpendotaTimedOut => "OPENDOTA_TIMED_OUT",
            DiagnosticCode::OpendotaRateLimited => "OPENDOTA_RATE_LIMITED",
            DiagnosticCode::OpendotaBadResponse => "OPENDOTA_BAD_RESPONSE",
            DiagnosticCode::Unclassified => "UNCLASSIFIED_ERROR",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Texto da interface: `msgid` em PT-BR com `{}` para cada argumento. A
/// tradução e a substituição dos argumentos ficam na apresentação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiText {
    pub msgid: &'static str,
    pub args: Vec<String>,
}

impl UiText {
    /// Texto sem argumentos.
    pub fn new(msgid: &'static str) -> Self {
        Self {
            msgid,
            args: Vec::new(),
        }
    }

    /// Texto com um argumento para cada `{}` do `msgid`, em ordem.
    pub fn with(msgid: &'static str, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            msgid,
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

/// Diagnóstico padronizado de uma falha, pronto para a interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    /// Identifica a falha ou a operação que não foi concluída.
    pub title: UiText,
    /// O que falhou, com o contexto conhecido.
    pub cause: UiText,
    /// Como resolver ou investigar.
    pub resolution: UiText,
    /// Cadeia de causas do erro original, sem dados sensíveis.
    pub technical_details: Option<String>,
}

impl Diagnostic {
    /// Diagnóstico de uma causa ainda não classificada. Mantém o erro original
    /// nos detalhes técnicos em vez de presumir uma causa conhecida.
    pub fn unclassified(
        title: UiText,
        context: impl Into<String>,
        error: &(dyn Error + 'static),
        redactor: &Redactor,
    ) -> Self {
        Self {
            code: DiagnosticCode::Unclassified,
            title,
            cause: UiText::with(
                "{} A causa ainda não foi classificada.",
                [redactor.redact(&context.into())],
            ),
            resolution: UiText::new("Consulte os detalhes técnicos para investigar a falha."),
            technical_details: Some(technical_details(error, redactor)),
        }
    }
}

/// Converte um erro específico no diagnóstico padronizado.
pub trait ToDiagnostic {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic;
}

/// Mensagem do erro seguida das mensagens de cada causa (`source`), separadas
/// por `: `, com os segredos removidos.
pub fn technical_details(error: &(dyn Error + 'static), redactor: &Redactor) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    redactor.redact(&chain)
}

/// Registra no log, como erro, um diagnóstico já sanitizado: só o código e os
/// detalhes técnicos, com a mensagem em inglês.
pub fn log_error(diagnostic: &Diagnostic, message: &str) {
    tracing::error!(
        code = %diagnostic.code,
        details = diagnostic.technical_details.as_deref().unwrap_or_default(),
        "{message}"
    );
}

/// Como `log_error`, para falhas em que o aplicativo segue com um fallback.
pub fn log_warning(diagnostic: &Diagnostic, message: &str) {
    tracing::warn!(
        code = %diagnostic.code,
        details = diagnostic.technical_details.as_deref().unwrap_or_default(),
        "{message}"
    );
}

/// Texto que substitui um segredo encontrado em uma mensagem.
pub const REDACTED: &str = "[removido]";

/// Remove segredos (como o token do GSI) de textos antes de exibi-los.
#[derive(Clone, Default)]
pub struct Redactor {
    secrets: Arc<[Arc<str>]>,
}

impl Redactor {
    /// Valores vazios são ignorados, porque substituí-los alteraria qualquer texto.
    pub fn new(secrets: impl IntoIterator<Item = impl Into<Arc<str>>>) -> Self {
        Self {
            secrets: secrets
                .into_iter()
                .map(Into::into)
                .filter(|secret: &Arc<str>| !secret.is_empty())
                .collect(),
        }
    }

    /// Texto com cada ocorrência de um segredo trocada por `REDACTED`.
    pub fn redact(&self, text: &str) -> String {
        self.secrets
            .iter()
            .fold(text.to_owned(), |redacted, secret| {
                redacted.replace(secret.as_ref(), REDACTED)
            })
    }
}

/// Mostra só a quantidade de segredos, para nunca expô-los em logs.
impl fmt::Debug for Redactor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Redactor")
            .field("secrets", &self.secrets.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/application/error_tests.rs"]
mod tests;
