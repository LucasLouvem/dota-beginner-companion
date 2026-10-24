//! Widgets e formatações reutilizados pelas páginas.

use std::time::{SystemTime, UNIX_EPOCH};

use relm4::adw::prelude::*;
use relm4::{adw, gtk};

use super::i18n::{translate, translate_with};

/// Gravidade de uma linha de status.
#[derive(Debug, Clone, Copy)]
pub enum Level {
    Ok,
    Warning,
    Error,
    Info,
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
