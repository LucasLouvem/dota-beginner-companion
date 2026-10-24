//! Textos da interface. Os msgids são em PT-BR; outras línguas entram por
//! arquivos `.po` em `po/`.

/// Domínio gettext das traduções do aplicativo.
pub const DOMAIN: &str = "dota-companion";

/// Deve ser chamada no início de `main`, antes de criar qualquer thread.
pub fn init() {
    use gettextrs::{
        LocaleCategory, bind_textdomain_codeset, bindtextdomain, setlocale, textdomain,
    };

    let locale_dir = option_env!("DOTA_COMPANION_LOCALEDIR").unwrap_or("/usr/share/locale");
    // SAFETY: chamada única antes de existirem outras threads.
    unsafe {
        setlocale(LocaleCategory::LcAll, "");
    }
    // Sem traduções instaladas o gettext devolve o próprio msgid (PT-BR).
    let _ = bindtextdomain(DOMAIN, locale_dir);
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
}

/// Texto traduzido para o idioma do sistema.
pub fn translate(msgid: &str) -> String {
    gettextrs::gettext(msgid)
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
