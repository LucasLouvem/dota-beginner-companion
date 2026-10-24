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
mod tests {
    use super::*;

    #[test]
    fn replaces_each_placeholder_with_the_next_argument() {
        assert_eq!(translate_with("{} de {}", &["1", "2"]), "1 de 2");
    }

    #[test]
    fn keeps_text_unchanged_when_there_are_no_placeholders() {
        assert_eq!(translate_with("sem marcador", &["x"]), "sem marcador");
    }

    #[test]
    fn keeps_extra_placeholders_when_arguments_run_out() {
        assert_eq!(translate_with("{} e {}", &["1"]), "1 e {}");
    }
}
