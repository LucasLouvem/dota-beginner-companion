//! Páginas da janela principal e a lista de abas.

pub mod diagnostics;

use relm4::adw;

use super::i18n::{translate, translate_with};

/// Página de status com título, descrição e a fase do plano que traz o conteúdo da aba.
pub fn planned_page(title: &str, phase: u8, description: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("content-loading-symbolic")
        .title(title)
        .description(format!(
            "{}\n\n{}",
            description,
            translate_with("Disponível na Fase {} do projeto.", &[&phase.to_string()])
        ))
        .build()
}

/// Abas da janela, exceto Diagnóstico: identificador, título e página.
pub fn tabs() -> Vec<(&'static str, String, adw::StatusPage)> {
    vec![
        (
            "hero",
            translate("Herói"),
            planned_page(
                &translate("Herói"),
                2,
                &translate("Mostrará o herói que você escolheu, atributos, funções e habilidades."),
            ),
        ),
        (
            "abilities",
            translate("Habilidades"),
            planned_page(
                &translate("Habilidades"),
                3,
                &translate("Mostrará a ordem de habilidades mais usada, com o tamanho da amostra."),
            ),
        ),
        (
            "items",
            translate("Itens"),
            planned_page(
                &translate("Itens"),
                3,
                &translate(
                    "Mostrará sugestões estatísticas de itens com frequência, momento de compra e fonte.",
                ),
            ),
        ),
        (
            "pro",
            translate("Dados profissionais"),
            planned_page(
                &translate("Dados profissionais"),
                3,
                &translate(
                    "Mostrará partidas profissionais recentes do patch atual usadas nas sugestões.",
                ),
            ),
        ),
        (
            "history",
            translate("Histórico"),
            planned_page(
                &translate("Histórico"),
                4,
                &translate("Mostrará suas partidas anteriores e o resumo pós-partida."),
            ),
        ),
        (
            "settings",
            translate("Configurações"),
            planned_page(
                &translate("Configurações"),
                2,
                &translate(
                    "Permitirá ajustar porta, bibliotecas Steam, chaves de API e limpar o cache.",
                ),
            ),
        ),
    ]
}
