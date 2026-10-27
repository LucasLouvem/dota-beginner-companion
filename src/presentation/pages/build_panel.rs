//! Widgets da página da build dentro da aba Herói: à esquerda a lista de
//! vitórias para escolher e, à direita, o cabeçalho do herói e os cards da
//! build, da partida e da popularidade. Os widgets são criados uma vez;
//! cada atualização mexe só no que mudou.
//!
//! Composição responsiva: cada grupo de cards é uma `CardGrid` cujo número de
//! colunas muda com a largura (`adw::BreakpointBin`). Em janela larga, a lista
//! fica à esquerda com largura fixa; em janela média, os grupos têm menos
//! colunas; em janela estreita, a lista vai para cima do conteúdo e cada
//! grupo vira uma coluna.

use std::rc::Rc;

use relm4::adw::prelude::*;
use relm4::gtk::gdk;
use relm4::{adw, gtk};

use crate::application::use_cases::item_icons::ItemIcons;
use crate::presentation::build_widgets::{AbilityGridView, CompareView, IconTextures, ItemStrip};
use crate::presentation::card_grid::{CardGrid, Columns, Layout};
use crate::presentation::i18n::translate;
use crate::presentation::widgets::{RowChange, RowContent, RowSection, boxed_list, list_changes};

use super::build::{BuildDetail, ItemChip, SidebarItem, kda_label, kda_text};
use super::build_cards::HeroHeader;

/// Largura da lista de builds em janela larga, em pixels.
const SIDEBAR_WIDTH: i32 = 230;
/// Altura máxima da lista antes de ela rolar sozinha.
const SIDEBAR_MAX_HEIGHT: i32 = 720;
/// Largura da página (sem o menu e as margens) em que a composição muda. O
/// limite vem da largura mínima da composição larga, conferida no app: com a
/// lista ao lado e quatro colunas a página pede ~850 px, e cada coluna só fica
/// confortável com ~220 px.
/// Até esta largura, os grupos de cards têm menos colunas.
const MEDIUM_LAYOUT: &str = "max-width: 1200sp";
/// A janela não encolhe abaixo de 760sp de página (ver `MIN_WINDOW_WIDTH`);
/// esta é a menor composição.
const MIN_PAGE_WIDTH: i32 = 760;
/// Espaço entre blocos e entre as colunas.
const GAP: i32 = 8;
/// Tamanho do retrato do herói, na proporção da imagem (16:9).
const PORTRAIT_WIDTH: i32 = 128;
const PORTRAIT_HEIGHT: i32 = 72;
/// Margem interna dos cards.
const CARD_PADDING: i32 = 10;

/// O que a página mostra num momento.
pub struct BuildView<'a> {
    pub hero: HeroHeader,
    /// Avisos e falhas, cada um num card.
    pub notices: Vec<RowContent>,
    pub sidebar: &'a [SidebarItem],
    /// Linha selecionada na lista.
    pub selected: Option<usize>,
    pub detail: Option<&'a BuildDetail>,
    pub icons: &'a ItemIcons,
}

/// Bloco com título e conteúdo.
struct Card {
    root: gtk::Box,
    body: gtk::Box,
}

impl Card {
    fn new(title: &str) -> Self {
        let heading = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .wrap(true)
            .build();
        heading.add_css_class("heading");
        heading.add_css_class("dota-card-title");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
        inner.set_margin_top(CARD_PADDING);
        inner.set_margin_bottom(CARD_PADDING);
        inner.set_margin_start(CARD_PADDING);
        inner.set_margin_end(CARD_PADDING);
        inner.append(&heading);
        inner.append(&body);
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .accessible_role(gtk::AccessibleRole::Group)
            .hexpand(true)
            .build();
        root.add_css_class("dota-card");
        root.append(&inner);
        root.update_property(&[gtk::accessible::Property::Label(title)]);
        Self { root, body }
    }
}

/// Faixa de itens com um subtítulo opcional, escondida quando vazia.
struct LabeledStrip {
    title: gtk::Label,
    strip: ItemStrip,
}

impl LabeledStrip {
    fn new(parent: &gtk::Box, title: &str) -> Self {
        let title = gtk::Label::builder().label(title).xalign(0.0).build();
        title.add_css_class("dim-label");
        title.add_css_class("caption-heading");
        let strip = ItemStrip::new();
        parent.append(&title);
        parent.append(strip.widget());
        Self { title, strip }
    }

    fn show(&mut self, chips: &[ItemChip], icons: &ItemIcons, textures: &mut IconTextures) {
        self.strip.show(chips, icons, textures);
        self.title
            .set_visible(!chips.is_empty() && !self.title.label().is_empty());
    }
}

/// Linha de rótulos curtos (atributo, funções e modo) que quebra a linha.
struct ChipRow {
    flow: gtk::FlowBox,
    labels: Vec<gtk::Label>,
    shown: Vec<String>,
}

impl ChipRow {
    fn new() -> Self {
        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .max_children_per_line(20)
            .row_spacing(6)
            .column_spacing(6)
            .halign(gtk::Align::Start)
            .build();
        Self {
            flow,
            labels: Vec::new(),
            shown: Vec::new(),
        }
    }

    fn show(&mut self, chips: &[String]) {
        for change in list_changes(&self.shown, chips, |_, _| true) {
            match change {
                RowChange::Update(index) | RowChange::Replace(index) => {
                    self.labels[index].set_label(&chips[index]);
                    self.labels[index].set_tooltip_text(Some(&chips[index]));
                }
                RowChange::Append(index) => {
                    let label = gtk::Label::builder()
                        .label(&chips[index])
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .max_width_chars(36)
                        .tooltip_text(&chips[index])
                        .build();
                    label.add_css_class("dota-chip-label");
                    label.add_css_class("caption");
                    let child = gtk::FlowBoxChild::new();
                    child.set_focusable(false);
                    child.set_child(Some(&label));
                    self.flow.append(&child);
                    self.labels.push(label);
                }
                RowChange::Truncate(len) => {
                    for label in self.labels.drain(len..) {
                        if let Some(child) = label.parent() {
                            self.flow.remove(&child);
                        }
                    }
                }
            }
        }
        self.shown = chips.to_vec();
        self.flow.set_visible(!chips.is_empty());
    }
}

/// Um card por aviso, na grade de avisos.
struct NoticeCards {
    grid: CardGrid,
    sections: Vec<RowSection>,
    shown: Vec<RowContent>,
}

impl NoticeCards {
    fn new(grid: CardGrid) -> Self {
        Self {
            grid,
            sections: Vec::new(),
            shown: Vec::new(),
        }
    }

    fn show(&mut self, notices: &[RowContent]) {
        for change in list_changes(&self.shown, notices, |_, _| true) {
            match change {
                RowChange::Update(index) | RowChange::Replace(index) => {
                    self.sections[index].show(vec![notices[index].clone()]);
                }
                RowChange::Append(index) => {
                    let list = boxed_list();
                    list.set_hexpand(true);
                    let mut section = RowSection::new(list.clone());
                    section.show(vec![notices[index].clone()]);
                    self.grid.push(&list, 1);
                    self.sections.push(section);
                }
                RowChange::Truncate(len) => {
                    self.sections.truncate(len);
                    self.grid.truncate(len);
                }
            }
        }
        self.shown = notices.to_vec();
        self.grid.widget().set_visible(!notices.is_empty());
    }
}

/// Uma linha da lista de builds.
struct SidebarRow {
    row: gtk::ListBoxRow,
    title: gtk::Label,
    subtitle: gtk::Label,
    /// K/D/A da vitória, à direita da linha.
    kda: gtk::Label,
}

impl SidebarRow {
    fn new(item: &SidebarItem) -> Self {
        let row = gtk::ListBoxRow::new();
        let title = gtk::Label::builder().xalign(0.0).wrap(true).build();
        let subtitle = gtk::Label::builder().xalign(0.0).wrap(true).build();
        subtitle.add_css_class("dim-label");
        subtitle.add_css_class("caption");
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        text.append(&title);
        text.append(&subtitle);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.set_margin_top(6);
        content.set_margin_bottom(6);
        content.set_margin_start(10);
        content.set_margin_end(10);
        let kda = gtk::Label::builder().valign(gtk::Align::Start).build();
        kda.add_css_class("dota-kda");
        kda.set_visible(false);
        content.append(&text);
        content.append(&kda);
        match item {
            SidebarItem::Entry(_) => row.add_css_class("dota-build-row"),
            SidebarItem::Group(_) => {
                title.add_css_class("heading");
                row.set_selectable(false);
                row.set_activatable(false);
            }
            SidebarItem::Empty(_) => {
                row.set_selectable(false);
                row.set_activatable(false);
            }
        }
        row.set_child(Some(&content));
        Self {
            row,
            title,
            subtitle,
            kda,
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
        let kda = match item {
            SidebarItem::Entry(entry) => entry.kda,
            _ => None,
        };
        let kda_text = kda.map(kda_text).unwrap_or_default();
        if self.kda.label() != kda_text {
            self.kda.set_label(&kda_text);
        }
        self.kda.set_visible(!kda_text.is_empty());
        let kda_label = kda.map(kda_label).unwrap_or_default();
        self.row.update_property(&[gtk::accessible::Property::Label(
            &[title, subtitle, kda_label.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
        )]);
    }
}

/// Grupos de cards da coluna principal, com as colunas de cada composição.
const NOTICE_COLUMNS: Columns = Columns::new(2, 2);
const ITEM_COLUMNS: Columns = Columns::new(3, 2);
const PROGRESS_COLUMNS: Columns = Columns::new(4, 3);

/// Posições dos cards em cada grade, para esconder os vazios.
const ITEMS_INVENTORY: usize = 0;
const ITEMS_STARTING: usize = 1;
const ITEMS_BACKPACK: usize = 2;
const PROGRESS_ORDER: usize = 0;
const PROGRESS_NEUTRAL: usize = 1;

/// Página da build.
pub struct BuildPanel {
    root: gtk::Box,
    found_button: gtk::Button,
    /// Retrato do herói; escondido até a imagem chegar.
    portrait: gtk::Picture,
    shown_portrait: Option<gdk::Texture>,
    hero_name: gtk::Label,
    hero_chips: ChipRow,
    hero_chips_title: gtk::Label,
    hero_meta: gtk::Label,
    notices: NoticeCards,
    sidebar_list: gtk::ListBox,
    sidebar_rows: Vec<SidebarRow>,
    sidebar_shown: Vec<SidebarItem>,
    items_grid: CardGrid,
    inventory: LabeledStrip,
    starting: LabeledStrip,
    backpack: LabeledStrip,
    progress_grid: CardGrid,
    compare_card: gtk::Box,
    order: LabeledStrip,
    neutral: LabeledStrip,
    enhancement: LabeledStrip,
    compare: CompareView,
    abilities_card: gtk::Box,
    grid: AbilityGridView,
    abilities_note: RowSection,
    textures: IconTextures,
}

/// Acompanha o ponto de quebra da composição média e reorganiza as grades.
fn track_layout(breakpoint: &adw::Breakpoint, grids: &Rc<Vec<CardGrid>>) {
    for applied in [true, false] {
        let grids = Rc::clone(grids);
        let update = move |_: &adw::Breakpoint| {
            let layout = if applied {
                Layout::Medium
            } else {
                Layout::Wide
            };
            for grid in grids.iter() {
                grid.set_layout(layout);
            }
        };
        if applied {
            breakpoint.connect_apply(update);
        } else {
            breakpoint.connect_unapply(update);
        }
    }
}

fn breakpoint(condition: &str) -> adw::Breakpoint {
    adw::Breakpoint::new(
        adw::BreakpointCondition::parse(condition)
            .expect("a condição de largura é uma constante válida"),
    )
}

impl BuildPanel {
    /// `on_select` recebe a posição da linha escolhida na lista; o botão "Partida encontrada" é criado pela página
    /// dona dos comandos.
    pub fn new(on_select: impl Fn(i32) + 'static, found_button: &gtk::Button) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, GAP);

        // Lista de builds, com rolagem própria.
        let sidebar_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        sidebar_list.add_css_class("boxed-list");
        sidebar_list.update_property(&[gtk::accessible::Property::Label(&translate(
            "Builds para escolher",
        ))]);
        sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                on_select(row.index());
            }
        });
        let sidebar_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(SIDEBAR_MAX_HEIGHT)
            .child(&sidebar_list)
            .build();
        let sidebar_heading = gtk::Label::builder()
            .label(translate("Seleção de build"))
            .xalign(0.0)
            .build();
        sidebar_heading.add_css_class("title-4");

        let sidebar_column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sidebar_column.set_width_request(SIDEBAR_WIDTH);
        sidebar_column.set_valign(gtk::Align::Start);
        sidebar_column.append(&sidebar_heading);
        sidebar_column.append(&sidebar_scroll);

        let main = gtk::Box::new(gtk::Orientation::Vertical, GAP);
        main.set_hexpand(true);

        // Cabeçalho do herói: nome, atributo e funções (dados fixos do
        // herói) e, em outro bloco, o modo de jogo (estado da partida).
        let hero_name = gtk::Label::builder().xalign(0.0).wrap(true).build();
        hero_name.add_css_class("title-2");
        hero_name.set_accessible_role(gtk::AccessibleRole::Heading);
        let hero_chips_title = gtk::Label::builder()
            .label(translate("Atributo e funções"))
            .xalign(0.0)
            .build();
        hero_chips_title.add_css_class("dim-label");
        hero_chips_title.add_css_class("caption-heading");
        let hero_chips = ChipRow::new();
        hero_chips
            .flow
            .update_property(&[gtk::accessible::Property::Label(&translate(
                "Atributo e funções do herói",
            ))]);
        let hero_meta = gtk::Label::builder().xalign(0.0).wrap(true).build();
        hero_meta.add_css_class("dim-label");
        hero_meta.add_css_class("caption");
        let hero_block = gtk::Box::new(gtk::Orientation::Vertical, 3);
        hero_block.append(&hero_name);
        hero_block.append(&hero_chips_title);
        hero_block.append(hero_chips.flow.upcast_ref::<gtk::Widget>());
        hero_block.append(&hero_meta);

        // O modo é perguntado numa janela quando a partida é encontrada; este
        // botão só marca a partida quando o aplicativo não a deduziu sozinho.
        found_button.set_halign(gtk::Align::Start);
        hero_block.append(found_button);
        let portrait = gtk::Picture::builder()
            .can_shrink(true)
            .valign(gtk::Align::Start)
            .width_request(PORTRAIT_WIDTH)
            .height_request(PORTRAIT_HEIGHT)
            .visible(false)
            .build();
        portrait.update_property(&[gtk::accessible::Property::Label(&translate(
            "Retrato do herói",
        ))]);
        hero_block.set_hexpand(true);
        // Você × build, no topo à direita, do outro lado do retrato.
        let compare_card = Card::new(&translate("Você × build"));
        let compare = CompareView::new();
        compare_card.body.append(compare.widget());
        compare_card.root.set_hexpand(false);
        compare_card.root.set_valign(gtk::Align::Start);
        let hero_row = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
        hero_row.append(&portrait);
        hero_row.append(&hero_block);
        hero_row.append(&compare_card.root);
        main.append(&hero_row);

        // Avisos, um card cada.
        let notice_grid = CardGrid::new(GAP, NOTICE_COLUMNS);
        main.append(notice_grid.widget());
        let notices = NoticeCards::new(notice_grid.clone());

        // Topo: resumo, qualidade dos dados e atenção.

        // Itens: inventário final, itens iniciais e mochila.
        let items_grid = CardGrid::new(GAP, ITEM_COLUMNS);
        let inventory_card = Card::new(&translate("Inventário final"));
        let inventory = LabeledStrip::new(&inventory_card.body, "");
        let starting_card = Card::new(&translate("Itens iniciais"));
        let starting = LabeledStrip::new(&starting_card.body, "");
        let backpack_card = Card::new(&translate("Mochila"));
        let backpack = LabeledStrip::new(&backpack_card.body, "");
        items_grid.push(&inventory_card.root, 1);
        items_grid.push(&starting_card.root, 1);
        items_grid.push(&backpack_card.root, 1);
        main.append(items_grid.widget());

        // Progresso: ordem de compra, item neutro e aprimoramento, e todas
        // as compras por fase.
        let progress_grid = CardGrid::new(GAP, PROGRESS_COLUMNS);
        let order_card = Card::new(&translate("Ordem de compra"));
        let legend = gtk::Label::builder()
            .label(translate(
                "✓ já comprado · → próximo item · esmaecido: ainda não comprado",
            ))
            .xalign(0.0)
            .wrap(true)
            .build();
        legend.add_css_class("dim-label");
        legend.add_css_class("caption");
        order_card.body.append(&legend);
        let order = LabeledStrip::new(&order_card.body, "");
        // O item neutro e o aprimoramento dele, lado a lado no mesmo card.
        let neutral_card = Card::new(&translate("Item neutro"));
        // `FlowBox`: em card estreito, o aprimoramento passa para baixo do item.
        let neutral_row = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .max_children_per_line(2)
            .row_spacing(GAP as u32)
            .column_spacing(GAP as u32)
            .halign(gtk::Align::Start)
            .build();
        let neutral_item = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let neutral = LabeledStrip::new(&neutral_item, &translate("Item"));
        let neutral_upgrade = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let enhancement = LabeledStrip::new(&neutral_upgrade, &translate("Aprimoramento"));
        for part in [&neutral_item, &neutral_upgrade] {
            let child = gtk::FlowBoxChild::new();
            child.set_focusable(false);
            child.set_child(Some(part));
            neutral_row.append(&child);
        }
        neutral_card.body.append(&neutral_row);
        progress_grid.push_spans(&order_card.root, Columns::new(3, 2));
        progress_grid.push(&neutral_card.root, 1);
        main.append(progress_grid.widget());

        // Ordem de habilidades, na largura toda.
        let abilities_card = Card::new(&translate("Ordem de habilidades"));
        let grid = AbilityGridView::new();
        let abilities_note_list = boxed_list();
        abilities_card.body.append(grid.widget());
        abilities_card.body.append(&abilities_note_list);
        main.append(&abilities_card.root);

        let columns = gtk::Box::new(gtk::Orientation::Horizontal, GAP + 6);
        columns.append(&sidebar_column);
        columns.append(&main);
        let bin = adw::BreakpointBin::builder()
            .width_request(MIN_PAGE_WIDTH)
            .height_request(200)
            .child(&columns)
            .build();
        // O ponto de quebra muda as colunas dos grupos pelo sinal `apply`.
        let grids = Rc::new(vec![notice_grid, items_grid.clone(), progress_grid.clone()]);
        let medium = breakpoint(MEDIUM_LAYOUT);
        track_layout(&medium, &grids);
        bin.add_breakpoint(medium);
        root.append(&bin);

        Self {
            root,
            found_button: found_button.clone(),
            portrait,
            shown_portrait: None,
            hero_name,
            hero_chips,
            hero_chips_title,
            hero_meta,
            notices,
            sidebar_list,
            sidebar_rows: Vec::new(),
            sidebar_shown: Vec::new(),
            items_grid,
            inventory,
            starting,
            backpack,
            progress_grid,
            compare_card: compare_card.root,
            order,
            neutral,
            enhancement,
            compare,
            abilities_card: abilities_card.root,
            grid,
            abilities_note: RowSection::new(abilities_note_list),
            textures: IconTextures::default(),
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show(&mut self, view: &BuildView<'_>) {
        self.show_header(&view.hero, view.icons);
        self.notices.show(&view.notices);
        self.show_sidebar(view.sidebar);
        self.select(view.selected);
        self.show_detail(view.detail, view.icons);
    }

    fn show_header(&mut self, hero: &HeroHeader, icons: &ItemIcons) {
        // Verde enquanto o jogador está numa partida; o estado também vai na
        // descrição, para não depender só da cor.
        if self.found_button.has_css_class("dota-match-active") != hero.in_match {
            if hero.in_match {
                self.found_button.add_css_class("dota-match-active");
            } else {
                self.found_button.remove_css_class("dota-match-active");
            }
            self.found_button
                .update_property(&[gtk::accessible::Property::Description(&if hero.in_match {
                    translate("Você está numa partida")
                } else {
                    String::new()
                })]);
        }
        let portrait = hero
            .image
            .as_deref()
            .and_then(|key| self.textures.get(key, icons));
        if self.shown_portrait != portrait {
            self.portrait.set_paintable(portrait.as_ref());
            self.portrait.set_visible(portrait.is_some());
            self.shown_portrait = portrait;
        }
        set_label(&self.hero_name, &hero.name);
        self.hero_chips.show(&hero.chips);
        self.hero_chips_title.set_visible(!hero.chips.is_empty());
        let meta = [&hero.source, &hero.last_match]
            .into_iter()
            .filter(|part| !part.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ");
        set_label(&self.hero_meta, &meta);
        self.hero_meta.set_visible(!meta.is_empty());
    }

    fn show_sidebar(&mut self, items: &[SidebarItem]) {
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
        self.sidebar_shown = items.to_vec();
    }

    /// Marca a linha escolhida, sem disparar a seleção se ela já está certa.
    /// O destaque é o da própria seleção (fundo e barra azul).
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
        let textures = &mut self.textures;
        self.order.show(&shown.order, icons, textures);
        self.starting.show(&shown.starting, icons, textures);
        self.inventory.show(&shown.finals, icons, textures);
        self.backpack.show(&shown.backpack, icons, textures);
        self.neutral.show(&shown.neutral, icons, textures);
        self.enhancement.show(&shown.enhancement, icons, textures);
        self.items_grid
            .set_visible(ITEMS_INVENTORY, !shown.finals.is_empty());
        self.items_grid
            .set_visible(ITEMS_STARTING, !shown.starting.is_empty());
        self.items_grid
            .set_visible(ITEMS_BACKPACK, !shown.backpack.is_empty());
        self.progress_grid
            .set_visible(PROGRESS_ORDER, !shown.order.is_empty());
        self.progress_grid.set_visible(
            PROGRESS_NEUTRAL,
            !shown.neutral.is_empty() || !shown.enhancement.is_empty(),
        );
        self.compare.show(shown.comparison.as_ref());
        self.compare_card.set_visible(shown.comparison.is_some());
        self.grid.show(shown.abilities.as_ref(), icons, textures);
        self.abilities_note
            .show(shown.abilities_note.iter().cloned().collect());
        self.abilities_card
            .set_visible(shown.abilities.is_some() || shown.abilities_note.is_some());
    }
}

/// Troca o texto só quando ele mudou.
fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}
