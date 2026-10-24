//! Textos da interface. Os msgids são em PT-BR; outras línguas entram por
//! arquivos `.po` em `po/`.

use std::io;

use crate::application::error::{
    Diagnostic, DiagnosticCode, Redactor, ToDiagnostic, UiText, technical_details,
};

/// Domínio gettext das traduções do aplicativo.
pub const DOMAIN: &str = "dota-companion";

/// Falha ao preparar as traduções. O aplicativo segue com os textos em PT-BR,
/// que são os próprios msgids.
#[derive(Debug, thiserror::Error)]
pub enum TranslationSetupError {
    #[error("o idioma do sistema não é suportado pela biblioteca C")]
    LocaleUnsupported,
    #[error("não foi possível indicar a pasta das traduções")]
    BindDomain(#[source] io::Error),
    #[error("não foi possível definir a codificação UTF-8 das traduções")]
    Codeset(#[source] io::Error),
    #[error("não foi possível ativar o domínio de traduções")]
    TextDomain(#[source] io::Error),
}

impl ToDiagnostic for TranslationSetupError {
    fn to_diagnostic(&self, redactor: &Redactor) -> Diagnostic {
        let (cause, resolution) = match self {
            TranslationSetupError::LocaleUnsupported => (
                "O idioma do sistema não está disponível; os textos ficam em português.",
                "Verifique se o idioma definido em LANG está instalado no sistema.",
            ),
            _ => (
                "As traduções não puderam ser carregadas; os textos ficam em português.",
                "Verifique a instalação das traduções do aplicativo.",
            ),
        };
        Diagnostic {
            code: DiagnosticCode::TranslationsUnavailable,
            title: UiText::new("Traduções indisponíveis"),
            cause: UiText::new(cause),
            resolution: UiText::new(resolution),
            technical_details: Some(technical_details(self, redactor)),
        }
    }
}

/// Deve ser chamada no início de `main`, antes de criar qualquer thread. Sem
/// traduções instaladas o gettext devolve o próprio msgid (PT-BR).
pub fn init() -> Result<(), TranslationSetupError> {
    use gettextrs::{
        LocaleCategory, bind_textdomain_codeset, bindtextdomain, setlocale, textdomain,
    };

    let locale_dir = option_env!("DOTA_COMPANION_LOCALEDIR").unwrap_or("/usr/share/locale");
    // SAFETY: chamada única na inicialização, antes do runtime Tokio e do GTK.
    // Nenhuma outra thread pode consultar ou alterar o locale durante a chamada.
    let locale = unsafe { setlocale(LocaleCategory::LcAll, "") };
    if locale.is_none() {
        return Err(TranslationSetupError::LocaleUnsupported);
    }
    bindtextdomain(DOMAIN, locale_dir).map_err(TranslationSetupError::BindDomain)?;
    bind_textdomain_codeset(DOMAIN, "UTF-8").map_err(TranslationSetupError::Codeset)?;
    textdomain(DOMAIN).map_err(TranslationSetupError::TextDomain)?;
    Ok(())
}

/// Texto traduzido para o idioma do sistema.
pub fn translate(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}

/// Traduz um texto definido fora da apresentação, como os de um `Diagnostic`.
pub fn translate_text(text: &UiText) -> String {
    let args: Vec<&str> = text.args.iter().map(String::as_str).collect();
    translate_with(text.msgid, &args)
}

/// Traduz e substitui cada `{}` pelos argumentos, em ordem.
pub fn translate_with(msgid: &str, args: &[&str]) -> String {
    let mut translated = String::new();
    let mut rest = translate(msgid);
    for arg in args {
        match rest.split_once("{}") {
            Some((before, after)) => {
                translated.push_str(before);
                translated.push_str(arg);
                rest = after.to_owned();
            }
            None => break,
        }
    }
    translated.push_str(&rest);
    translated
}

#[cfg(test)]
#[path = "../../tests/unit/presentation/i18n_tests.rs"]
mod tests;
