//! Páginas da janela principal e a lista de abas.

pub mod diagnostics;

use relm4::adw;

use super::i18n::{t, tf};

/// Página de status com título, descrição e a fase do plano que traz o conteúdo da aba.
pub fn planned_page(title: &str, phase: u8, description: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("content-loading-symbolic")
        .title(title)
        .description(format!(
            "{}\n\n{}",
            description,
            tf("Disponível na Fase {} do projeto.", &[&phase.to_string()])
        ))
        .build()
}

/// Abas da janela, exceto Diagnóstico: identificador, título e página.
pub fn tabs() -> Vec<(&'static str, String, adw::StatusPage)> {
    vec![
        (
            "hero",
            t("Herói"),
            planned_page(
                &t("Herói"),
                2,
                &t("Mostrará o herói que você escolheu, atributos, funções e habilidades."),
            ),
        ),
        (
            "abilities",
            t("Habilidades"),
            planned_page(
                &t("Habilidades"),
                3,
                &t("Mostrará a ordem de habilidades mais usada, com o tamanho da amostra."),
            ),
        ),
        (
            "items",
            t("Itens"),
            planned_page(
                &t("Itens"),
                3,
                &t(
                    "Mostrará sugestões estatísticas de itens com frequência, momento de compra e fonte.",
                ),
            ),
        ),
        (
            "pro",
            t("Dados profissionais"),
            planned_page(
                &t("Dados profissionais"),
                3,
                &t("Mostrará partidas profissionais recentes do patch atual usadas nas sugestões."),
            ),
        ),
        (
            "history",
            t("Histórico"),
            planned_page(
                &t("Histórico"),
                4,
                &t("Mostrará suas partidas anteriores e o resumo pós-partida."),
            ),
        ),
        (
            "settings",
            t("Configurações"),
            planned_page(
                &t("Configurações"),
                2,
                &t("Permitirá ajustar porta, bibliotecas Steam, chaves de API e limpar o cache."),
            ),
        ),
    ]
}
