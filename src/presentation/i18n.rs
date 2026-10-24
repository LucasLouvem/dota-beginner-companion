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
pub fn t(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}

/// Traduz e substitui cada `{}` pelos argumentos, em ordem.
pub fn tf(msgid: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = t(msgid);
    for arg in args {
        match rest.split_once("{}") {
            Some((before, after)) => {
                out.push_str(before);
                out.push_str(arg);
                rest = after.to_owned();
            }
            None => break,
        }
    }
    out.push_str(&rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tf_substitutes_in_order() {
        assert_eq!(tf("{} de {}", &["1", "2"]), "1 de 2");
        assert_eq!(tf("sem marcador", &["x"]), "sem marcador");
    }
}
