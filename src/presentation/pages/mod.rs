//! Páginas da janela principal e a lista de abas.

pub mod diagnostics;

use relm4::adw;

use super::i18n::translate;

/// Texto exibido nas abas cujo conteúdo ainda não está disponível.
const UNAVAILABLE_MESSAGE: &str = "Esta funcionalidade ainda não está disponível";

/// Página de status com título e indicação de indisponibilidade.
pub fn planned_page(title: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("content-loading-symbolic")
        .title(title)
        .description(translate(UNAVAILABLE_MESSAGE))
        .build()
}

/// Abas da janela, exceto Diagnóstico: identificador, título e página.
pub fn tabs() -> Vec<(&'static str, String, adw::StatusPage)> {
    [
        ("hero", "Herói"),
        ("abilities", "Habilidades"),
        ("items", "Itens"),
        ("pro", "Dados profissionais"),
        ("history", "Histórico"),
        ("settings", "Configurações"),
    ]
    .into_iter()
    .map(|(id, title)| {
        let title = translate(title);
        let page = planned_page(&title);
        (id, title, page)
    })
    .collect()
}
