//! Páginas da janela principal e a lista de abas.

pub mod build;
pub mod build_panel;
pub mod diagnostics;
pub mod hero;
pub mod pro;
pub mod recommendation_page;

use relm4::adw;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::error::SendError;

use crate::application::event_bus::{CoreConnectionError, UiCommand};

use super::i18n::translate;

/// Texto exibido nas abas cujo conteúdo ainda não está disponível.
const UNAVAILABLE_MESSAGE: &str = "Esta funcionalidade ainda não está disponível";

/// Envia o comando ao núcleo. O canal só fecha se o núcleo parou.
pub fn send_command(
    commands: &UnboundedSender<UiCommand>,
    command: UiCommand,
) -> Result<(), CoreConnectionError> {
    commands
        .send(command)
        .map_err(|SendError(command)| CoreConnectionError::CommandRejected { command })
}

/// Página de status com título e indicação de indisponibilidade.
pub fn planned_page(title: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("content-loading-symbolic")
        .title(title)
        .description(translate(UNAVAILABLE_MESSAGE))
        .build()
}

/// Abas ainda sem conteúdo: identificador, título e página.
pub fn tabs() -> Vec<(&'static str, String, adw::StatusPage)> {
    [("history", "Histórico"), ("settings", "Configurações")]
        .into_iter()
        .map(|(id, title)| {
            let title = translate(title);
            let page = planned_page(&title);
            (id, title, page)
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/mod_tests.rs"]
mod tests;
