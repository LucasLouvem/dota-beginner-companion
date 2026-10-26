//! Widgets da página da build dentro da aba Herói: o cabeçalho das
//! recomendações, a lista de vitórias à esquerda e, à direita, a build
//! escolhida com o progresso da partida. Os widgets são criados uma vez; cada
//! atualização mexe só no que mudou.

use relm4::adw::prelude::*;
use relm4::{adw, gtk};

use crate::application::use_cases::item_icons::ItemIcons;
use crate::presentation::build_widgets::{AbilityGridView, IconTextures, ItemStrip};
use crate::presentation::i18n::translate;
use crate::presentation::widgets::{
    Level, RowChange, RowContent, RowSection, boxed_list, list_changes, section_title,
};

use super::build::{BuildDetail, ItemChip, SidebarItem};

/// Largura da lista lateral, em pixels.
const SIDEBAR_WIDTH: i32 = 280;
/// Abaixo desta largura, a lista fica em cima da build.
const NARROW_LAYOUT: &str = "max-width: 720sp";
/// Lado da imagem dos itens na lista lateral.
const SIDEBAR_ICON_SIZE: i32 = 24;
/// Janelas de compra mostradas (antes do início e as três do jogo).
const PURCHASE_WINDOWS: usize = 4;

/// O que a página mostra num momento.
pub struct BuildView<'a> {
    pub header: Vec<RowContent>,
    pub sidebar: &'a [SidebarItem],
    /// Linha selecionada na lista.
    pub selected: Option<usize>,
    pub detail: Option<&'a BuildDetail>,
    pub progress: RowContent,
    pub popularity: Vec<RowContent>,
    pub icons: &'a ItemIcons,
}

/// Uma linha da lista lateral.
struct SidebarRow {
    row: gtk::ListBoxRow,
    title: gtk::Label,
    subtitle: gtk::Label,
    strip: Option<ItemStrip>,
}

impl SidebarRow {
    fn new(item: &SidebarItem) -> Self {
        let row = gtk::ListBoxRow::new();
        let title = gtk::Label::builder().xalign(0.0).wrap(true).build();
        let subtitle = gtk::Label::builder().xalign(0.0).wrap(true).build();
        subtitle.add_css_class("dim-label");
        subtitle.add_css_class("caption");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(10);
        content.set_margin_end(10);
        content.append(&title);
        content.append(&subtitle);
        let strip = match item {
            SidebarItem::Entry(_) => {
                let strip = ItemStrip::with_icon_size(SIDEBAR_ICON_SIZE);
                content.append(strip.widget());
                Some(strip)
            }
            SidebarItem::Group(_) => {
                title.add_css_class("heading");
                row.set_selectable(false);
                row.set_activatable(false);
                None
            }
            SidebarItem::Empty(_) => {
                row.set_selectable(false);
                row.set_activatable(false);
                None
            }
        };
        row.set_child(Some(&content));
        Self {
            row,
            title,
            subtitle,
            strip,
        }
    }

    fn show(&self, item: &SidebarItem) {
        let (title, subtitle) = match item {
            SidebarItem::Group(title) => (title.as_str(), ""),
            SidebarItem::Entry(entry) => (entry.title.as_str(), entry.subtitle.as_str()),
            SidebarItem::Empty(row) => (row.title.as_str(), row.subtitle.as_str()),
        };
        if self.title.label() != title {
            self.title.set_label(title);
        }
        if self.subtitle.label() != subtitle {
            self.subtitle.set_label(subtitle);
        }
        self.subtitle.set_visible(!subtitle.is_empty());
        self.row.update_property(&[gtk::accessible::Property::Label(
            &[title, subtitle]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
        )]);
    }
}

/// Faixa de itens com título, escondida quando vazia.
struct LabeledStrip {
    title: gtk::Label,
    strip: ItemStrip,
}

impl LabeledStrip {
    fn new(parent: &gtk::Box, title: &str) -> Self {
        let title = section_title(title);
        let strip = ItemStrip::new();
        parent.append(&title);
        parent.append(strip.widget());
        Self { title, strip }
    }

    fn show(&mut self, chips: &[ItemChip], icons: &ItemIcons, textures: &mut IconTextures) {
        self.strip.show(chips, icons, textures);
        self.title.set_visible(!chips.is_empty());
    }
}

/// Página da build.
pub struct BuildPanel {
    root: gtk::Box,
    header: RowSection,
    sidebar_list: gtk::ListBox,
    sidebar_rows: Vec<SidebarRow>,
    sidebar_shown: Vec<SidebarItem>,
    progress: RowSection,
    notes: RowSection,
    starting: LabeledStrip,
    order: LabeledStrip,
    finals: LabeledStrip,
    backpack: LabeledStrip,
    neutral: LabeledStrip,
    purchases: gtk::Expander,
    windows: Vec<LabeledStrip>,
    grid: AbilityGridView,
    abilities_note: RowSection,
    popularity: RowSection,
    textures: IconTextures,
}

impl BuildPanel {
    /// `on_select` recebe a posição da linha escolhida na lista.
    pub fn new(on_select: impl Fn(i32) + 'static) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let header_list = boxed_list();
        root.append(&header_list);

        let sidebar_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .width_request(SIDEBAR_WIDTH)
            .valign(gtk::Align::Start)
            .build();
        sidebar_list.add_css_class("boxed-list");
        sidebar_list.update_property(&[gtk::accessible::Property::Label(&translate(
            "Vitórias para escolher a build",
        ))]);
        sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                on_select(row.index());
            }
        });

        let main = gtk::Box::new(gtk::Orientation::Vertical, 8);
        main.set_hexpand(true);
        let progress_list = boxed_list();
        let notes_list = boxed_list();
        main.append(&progress_list);
        main.append(&notes_list);
        let starting = LabeledStrip::new(&main, &translate("Itens iniciais"));
        let order = LabeledStrip::new(&main, &translate("Ordem da build"));
        let finals = LabeledStrip::new(&main, &translate("Itens finais"));
        let backpack = LabeledStrip::new(&main, &translate("Mochila"));
        let neutral = LabeledStrip::new(&main, &translate("Item neutro e aprimoramento"));

        let windows_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let windows = (0..PURCHASE_WINDOWS)
            .map(|_| LabeledStrip::new(&windows_box, ""))
            .collect();
        let purchases = gtk::Expander::builder()
            .label(translate("Todas as compras, por fase"))
            .child(&windows_box)
            .build();
        main.append(&purchases);

        main.append(&section_title(&translate("Ordem de habilidades")));
        let grid = AbilityGridView::new();
        main.append(grid.widget());
        let abilities_note_list = boxed_list();
        main.append(&abilities_note_list);

        let columns = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        columns.append(&sidebar_list);
        columns.append(&main);
        let bin = adw::BreakpointBin::builder()
            .width_request(320)
            .height_request(200)
            .child(&columns)
            .build();
        let narrow = adw::Breakpoint::new(
            adw::BreakpointCondition::parse(NARROW_LAYOUT)
                .expect("a condição de largura é uma constante válida"),
        );
        narrow.add_setter(
            &columns,
            "orientation",
            Some(&gtk::Orientation::Vertical.to_value()),
        );
        narrow.add_setter(&sidebar_list, "width-request", Some(&(-1).to_value()));
        bin.add_breakpoint(narrow);
        root.append(&bin);

        let popularity_list = boxed_list();
        root.append(&section_title(&translate(
            "Itens mais comprados por fase (profissionais)",
        )));
        root.append(&popularity_list);

        Self {
            root,
            header: RowSection::new(header_list),
            sidebar_list,
            sidebar_rows: Vec::new(),
            sidebar_shown: Vec::new(),
            progress: RowSection::new(progress_list),
            notes: RowSection::new(notes_list),
            starting,
            order,
            finals,
            backpack,
            neutral,
            purchases,
            windows,
            grid,
            abilities_note: RowSection::new(abilities_note_list),
            popularity: RowSection::new(popularity_list),
            textures: IconTextures::default(),
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show(&mut self, view: &BuildView<'_>) {
        self.header.show(view.header.clone());
        self.show_sidebar(view.sidebar, view.icons);
        self.select(view.selected);
        self.progress.show(vec![view.progress.clone()]);
        self.show_detail(view.detail, view.icons);
        self.popularity.show(view.popularity.clone());
    }

    fn show_sidebar(&mut self, items: &[SidebarItem], icons: &ItemIcons) {
        let same_kind = |old: &SidebarItem, new: &SidebarItem| {
            std::mem::discriminant(old) == std::mem::discriminant(new)
        };
        for change in list_changes(&self.sidebar_shown, items, same_kind) {
            match change {
                RowChange::Update(index) => self.sidebar_rows[index].show(&items[index]),
                RowChange::Replace(index) => {
                    let row = SidebarRow::new(&items[index]);
                    row.show(&items[index]);
                    self.sidebar_list.insert(&row.row, index as i32);
                    self.sidebar_list.remove(&self.sidebar_rows[index].row);
                    self.sidebar_rows[index] = row;
                }
                RowChange::Append(index) => {
                    let row = SidebarRow::new(&items[index]);
                    row.show(&items[index]);
                    self.sidebar_list.append(&row.row);
                    self.sidebar_rows.push(row);
                }
                RowChange::Truncate(len) => {
                    for row in self.sidebar_rows.drain(len..) {
                        self.sidebar_list.remove(&row.row);
                    }
                }
            }
        }
        // As imagens podem chegar depois do texto, então toda faixa confere;
        // a faixa só mexe nos itens que mudaram ou ganharam imagem.
        for (row, item) in self.sidebar_rows.iter_mut().zip(items) {
            if let (Some(strip), SidebarItem::Entry(entry)) = (&mut row.strip, item) {
                strip.show(&entry.items, icons, &mut self.textures);
            }
        }
        self.sidebar_shown = items.to_vec();
    }

    /// Marca a linha escolhida, sem disparar a seleção se ela já está certa.
    fn select(&self, selected: Option<usize>) {
        let current = self.sidebar_list.selected_row().map(|row| row.index());
        let wanted = selected.and_then(|index| self.sidebar_rows.get(index));
        match wanted {
            Some(row) if current != Some(row.row.index()) => {
                self.sidebar_list.select_row(Some(&row.row));
            }
            None if current.is_some() => self.sidebar_list.unselect_all(),
            _ => {}
        }
    }

    fn show_detail(&mut self, detail: Option<&BuildDetail>, icons: &ItemIcons) {
        let empty = BuildDetail::default();
        let shown = detail.unwrap_or(&empty);
        let mut notes = shown.notes.clone();
        if detail.is_none() {
            notes.push(RowContent::new(
                Level::Info,
                &translate("Nenhuma build escolhida"),
                &translate("Escolha uma vitória na lista para ver os itens e as habilidades."),
            ));
        }
        self.notes.show(notes);
        let textures = &mut self.textures;
        self.starting.show(&shown.starting, icons, textures);
        self.order.show(&shown.order, icons, textures);
        self.finals.show(&shown.finals, icons, textures);
        self.backpack.show(&shown.backpack, icons, textures);
        self.neutral.show(&shown.neutral, icons, textures);
        for (index, window) in self.windows.iter_mut().enumerate() {
            match shown.windows.get(index) {
                Some((title, chips)) => {
                    if window.title.label() != title.as_str() {
                        window.title.set_label(title);
                    }
                    window.show(chips, icons, textures);
                }
                None => window.show(&[], icons, textures),
            }
        }
        self.purchases.set_visible(!shown.windows.is_empty());
        self.grid.show(shown.abilities.as_ref());
        self.abilities_note
            .show(shown.abilities_note.iter().cloned().collect());
    }
}
