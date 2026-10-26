//! Widgets e formatações reutilizados pelas páginas.

use std::time::{SystemTime, UNIX_EPOCH};

use relm4::adw::prelude::*;
use relm4::{adw, gtk};

use crate::application::error::Diagnostic;

use super::i18n::{translate, translate_text, translate_with};

/// Gravidade de uma linha de status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warning,
    Error,
    Info,
}

/// Conteúdo de uma linha de status, para comparar o que já está na tela com
/// o novo estado antes de mexer nos widgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowContent {
    pub level: Level,
    pub title: String,
    pub subtitle: String,
}

impl RowContent {
    pub fn new(level: Level, title: &str, subtitle: &str) -> Self {
        RowContent {
            level,
            title: title.to_owned(),
            subtitle: subtitle.to_owned(),
        }
    }

    /// Linha de erro de um diagnóstico: título, causa e resolução, código e
    /// detalhes técnicos, todos selecionáveis para copiar.
    pub fn diagnostic(diagnostic: &Diagnostic) -> Self {
        RowContent {
            level: Level::Error,
            title: translate_text(&diagnostic.title),
            subtitle: diagnostic_subtitle(diagnostic),
        }
    }

    /// Cria a linha de status com este conteúdo.
    pub fn to_row(&self) -> adw::ActionRow {
        status_row(self.level, &self.title, &self.subtitle)
    }
}

/// Alteração em uma lista de linhas, pela posição da linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowChange {
    /// Mesmo ícone; título ou subtítulo mudou.
    Update(usize),
    /// O nível mudou, e com ele o ícone: a linha é trocada por uma nova.
    Replace(usize),
    /// Linha nova no fim da lista.
    Append(usize),
    /// A lista fica só com as primeiras linhas, até esta quantidade.
    Truncate(usize),
}

/// O que muda, posição por posição, para a lista passar de `shown` a `new`.
/// Linhas iguais não geram alteração.
pub fn row_changes(shown: &[RowContent], new: &[RowContent]) -> Vec<RowChange> {
    list_changes(shown, new, |old_row, new_row| {
        old_row.level == new_row.level
    })
}

/// Como `row_changes`, para qualquer lista: `same_widget` diz se o elemento
/// mudado pode ser atualizado no mesmo widget (`Update`) ou precisa de um
/// novo (`Replace`).
pub fn list_changes<T: PartialEq>(
    shown: &[T],
    new: &[T],
    same_widget: impl Fn(&T, &T) -> bool,
) -> Vec<RowChange> {
    let mut changes: Vec<RowChange> = shown
        .iter()
        .zip(new)
        .enumerate()
        .filter(|(_, (old_item, new_item))| old_item != new_item)
        .map(|(index, (old_item, new_item))| {
            if same_widget(old_item, new_item) {
                RowChange::Update(index)
            } else {
                RowChange::Replace(index)
            }
        })
        .collect();
    changes.extend((shown.len()..new.len()).map(RowChange::Append));
    if new.len() < shown.len() {
        changes.push(RowChange::Truncate(new.len()));
    }
    changes
}

/// Lista de linhas de status que altera só as linhas que mudaram.
pub struct RowSection {
    list: gtk::ListBox,
    /// Linhas na ordem da lista, uma para cada item de `shown`.
    rows: Vec<adw::ActionRow>,
    shown: Vec<RowContent>,
}

impl RowSection {
    pub fn new(list: gtk::ListBox) -> Self {
        RowSection {
            list,
            rows: Vec::new(),
            shown: Vec::new(),
        }
    }

    /// Mostra `content`, mexendo só nas linhas que mudaram.
    pub fn show(&mut self, content: Vec<RowContent>) {
        for change in row_changes(&self.shown, &content) {
            match change {
                RowChange::Update(index) => {
                    let (row, old, new) = (&self.rows[index], &self.shown[index], &content[index]);
                    if old.title != new.title {
                        row.set_title(&new.title);
                    }
                    if old.subtitle != new.subtitle {
                        row.set_subtitle(&new.subtitle);
                    }
                }
                RowChange::Replace(index) => {
                    let row = content[index].to_row();
                    // A nova linha entra antes da antiga, que sai em seguida.
                    self.list.insert(&row, index as i32);
                    self.list.remove(&self.rows[index]);
                    self.rows[index] = row;
                }
                RowChange::Append(index) => {
                    let row = content[index].to_row();
                    self.list.append(&row);
                    self.rows.push(row);
                }
                RowChange::Truncate(len) => {
                    for row in self.rows.drain(len..) {
                        self.list.remove(&row);
                    }
                }
            }
        }
        self.shown = content;
    }
}

/// Linha de status com ícone, título e subtítulo selecionável (para copiar caminhos).
pub fn status_row(level: Level, title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .use_markup(false)
        .subtitle_selectable(true)
        .build();
    let (icon, class, tooltip) = match level {
        Level::Ok => ("emblem-ok-symbolic", "success", translate("OK")),
        Level::Warning => ("dialog-warning-symbolic", "warning", translate("Atenção")),
        Level::Error => ("dialog-error-symbolic", "error", translate("Erro")),
        Level::Info => (
            "dialog-information-symbolic",
            "dim-label",
            translate("Informação"),
        ),
    };
    let image = gtk::Image::from_icon_name(icon);
    image.add_css_class(class);
    image.set_tooltip_text(Some(&tooltip));
    row.add_prefix(&image);
    row
}

/// Diagnóstico completo em texto, com o título na primeira linha, para o terminal.
pub fn diagnostic_text(diagnostic: &Diagnostic) -> String {
    format!(
        "{}\n{}",
        translate_text(&diagnostic.title),
        diagnostic_subtitle(diagnostic)
    )
}

fn diagnostic_subtitle(diagnostic: &Diagnostic) -> String {
    let mut lines = vec![
        format!(
            "{} {}",
            translate_text(&diagnostic.cause),
            translate_text(&diagnostic.resolution)
        ),
        translate_with("Código: {}", &[diagnostic.code.as_str()]),
    ];
    if let Some(details) = &diagnostic.technical_details {
        lines.push(translate_with("Detalhes técnicos: {}", &[details]));
    }
    lines.join("\n")
}

/// Título de seção de uma página.
pub fn section_title(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("heading");
    label.set_xalign(0.0);
    label
}

/// Lista no estilo de cartão, sem seleção.
pub fn boxed_list() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

/// Duração no formato do relógio do Dota: `23:49` ou `1:02:03`.
pub fn format_duration(total_secs: u32) -> String {
    let (hours, minutes, seconds) = (total_secs / 3600, total_secs / 60 % 60, total_secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Data no fuso local, como `25/10/2026`; vazia se o horário não for válido.
pub fn format_date(unix_seconds: i64) -> String {
    relm4::gtk::glib::DateTime::from_unix_local(unix_seconds)
        .and_then(|local_time| local_time.format("%d/%m/%Y"))
        .map(|formatted| formatted.to_string())
        .unwrap_or_default()
}

/// "14:03:22 (há 5 s)" no fuso local.
pub fn format_time(at: SystemTime) -> String {
    let secs = at
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default();
    let clock = relm4::gtk::glib::DateTime::from_unix_local(secs)
        .and_then(|local_time| local_time.format("%H:%M:%S"))
        .map(|formatted| formatted.to_string())
        .unwrap_or_default();
    let ago = SystemTime::now()
        .duration_since(at)
        .unwrap_or_default()
        .as_secs();
    let relative = match ago {
        0..60 => translate_with("há {} s", &[&ago.to_string()]),
        60..3600 => translate_with("há {} min", &[&(ago / 60).to_string()]),
        _ => translate_with("há {} h", &[&(ago / 3600).to_string()]),
    };
    format!("{clock} ({relative})")
}

#[cfg(test)]
#[path = "../../tests/unit/presentation/widgets_tests.rs"]
mod tests;
