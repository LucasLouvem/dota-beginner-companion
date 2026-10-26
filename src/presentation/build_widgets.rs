//! Widgets da página da build: a faixa de itens com imagem e a grade da
//! ordem de habilidades. Os dois criam os widgets uma vez e, nas
//! atualizações, só trocam o que mudou, sem `remove_all()`.

use std::collections::HashMap;

use relm4::gtk::{self, gdk, glib, prelude::*};

use crate::application::use_cases::item_icons::ItemIcons;
use crate::domain::build_progress::StepState;

use super::pages::build::{AbilityGrid, ItemChip};
use super::widgets::{RowChange, list_changes};

/// Lado da imagem do item, em pixels.
const ICON_SIZE: i32 = 40;

/// Texturas das imagens dos itens, criadas uma vez por item a partir dos
/// bytes publicados no estado.
#[derive(Default)]
pub struct IconTextures {
    textures: HashMap<String, gdk::Texture>,
}

impl IconTextures {
    /// Textura do item; `None` quando a imagem ainda não chegou ou não é
    /// válida (o item fica só com o nome).
    pub fn get(&mut self, item: &str, icons: &ItemIcons) -> Option<gdk::Texture> {
        if let Some(texture) = self.textures.get(item) {
            return Some(texture.clone());
        }
        let bytes = icons.get(item)?;
        let texture = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes[..]))
            .inspect_err(|texture_error| {
                tracing::warn!(item, error = %texture_error, "invalid item icon");
            })
            .ok()?;
        self.textures.insert(item.to_owned(), texture.clone());
        Some(texture)
    }
}

/// Um item na faixa: imagem (ou o nome, sem imagem) e, embaixo, o símbolo da
/// situação e o minuto.
struct ChipWidget {
    child: gtk::FlowBoxChild,
    frame: gtk::Box,
    image: gtk::Image,
    name: gtk::Label,
    caption: gtk::Label,
}

impl ChipWidget {
    fn new() -> Self {
        let image = gtk::Image::builder().pixel_size(ICON_SIZE).build();
        let name = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(10)
            .justify(gtk::Justification::Center)
            .build();
        let caption = gtk::Label::new(None);
        caption.add_css_class("caption");
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 2);
        frame.set_margin_top(4);
        frame.set_margin_bottom(4);
        frame.set_margin_start(4);
        frame.set_margin_end(4);
        frame.append(&image);
        frame.append(&name);
        frame.append(&caption);
        let child = gtk::FlowBoxChild::new();
        child.set_child(Some(&frame));
        Self {
            child,
            frame,
            image,
            name,
            caption,
        }
    }

    fn show(&self, chip: &ItemChip, texture: Option<&gdk::Texture>) {
        match texture {
            Some(texture) => {
                self.image.set_paintable(Some(texture));
                self.image.set_visible(true);
                self.name.set_visible(false);
            }
            None => {
                self.image.set_visible(false);
                self.name.set_label(&chip.label);
                self.name.set_visible(true);
            }
        }
        self.caption.set_label(&chip_caption(chip));
        self.caption.set_visible(!self.caption.label().is_empty());
        for (class, active) in [
            ("success", chip.state == Some(StepState::Done)),
            ("accent", chip.state == Some(StepState::Next)),
            ("dim-label", chip.state == Some(StepState::Pending)),
        ] {
            if active {
                self.caption.add_css_class(class);
            } else {
                self.caption.remove_css_class(class);
            }
        }
        if chip.state == Some(StepState::Next) {
            self.frame.add_css_class("card");
        } else {
            self.frame.remove_css_class("card");
        }
        let label = chip.accessible_label();
        self.child.set_tooltip_text(Some(&label));
        self.child
            .update_property(&[gtk::accessible::Property::Label(&label)]);
    }
}

/// Texto embaixo da imagem: o símbolo da situação e o minuto da compra.
pub fn chip_caption(chip: &ItemChip) -> String {
    [chip.marker(), chip.time.as_deref().unwrap_or_default()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Faixa de itens que quebra a linha quando falta espaço.
pub struct ItemStrip {
    flow: gtk::FlowBox,
    chips: Vec<ChipWidget>,
    /// O que cada chip mostra agora, com a presença da imagem.
    shown: Vec<(ItemChip, bool)>,
}

impl ItemStrip {
    pub fn new() -> Self {
        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .max_children_per_line(30)
            .row_spacing(4)
            .column_spacing(4)
            .build();
        Self {
            flow,
            chips: Vec::new(),
            shown: Vec::new(),
        }
    }

    pub fn widget(&self) -> &gtk::FlowBox {
        &self.flow
    }

    /// Mostra `chips`, mexendo só nos que mudaram ou ganharam imagem.
    pub fn show(&mut self, chips: &[ItemChip], icons: &ItemIcons, textures: &mut IconTextures) {
        let content: Vec<(ItemChip, Option<gdk::Texture>)> = chips
            .iter()
            .map(|chip| (chip.clone(), textures.get(&chip.item, icons)))
            .collect();
        let new: Vec<(ItemChip, bool)> = content
            .iter()
            .map(|(chip, texture)| (chip.clone(), texture.is_some()))
            .collect();
        for change in list_changes(&self.shown, &new, |_, _| true) {
            match change {
                RowChange::Update(index) | RowChange::Replace(index) => {
                    let (chip, texture) = &content[index];
                    self.chips[index].show(chip, texture.as_ref());
                }
                RowChange::Append(index) => {
                    let (chip, texture) = &content[index];
                    let widget = ChipWidget::new();
                    widget.show(chip, texture.as_ref());
                    self.flow.append(&widget.child);
                    self.chips.push(widget);
                }
                RowChange::Truncate(len) => {
                    for widget in self.chips.drain(len..) {
                        self.flow.remove(&widget.child);
                    }
                }
            }
        }
        self.shown = new;
        self.flow.set_visible(!self.shown.is_empty());
    }
}

impl Default for ItemStrip {
    fn default() -> Self {
        Self::new()
    }
}

/// Texto de cada célula da grade: em cada coluna (ponto), só a linha da
/// habilidade evoluída tem marca.
pub fn grid_cells(grid: &AbilityGrid) -> Vec<Vec<&'static str>> {
    let mut cells = vec![vec![""; grid.points.len()]; grid.rows.len()];
    for (column, (row, state)) in grid.points.iter().enumerate() {
        if let Some(cell) = cells.get_mut(*row).and_then(|row| row.get_mut(column)) {
            *cell = match state {
                StepState::Done => "✓",
                StepState::Next => "→",
                StepState::Pending => "●",
            };
        }
    }
    cells
}

/// Grade da ordem de habilidades. As células são criadas quando a forma da
/// grade muda (outra build) e, no resto do tempo, só têm o texto e as
/// classes trocados.
pub struct AbilityGridView {
    root: gtk::Box,
    max_order: gtk::Label,
    grid: gtk::Grid,
    /// Células por linha e coluna, sem a linha de cabeçalho e a coluna de
    /// nomes.
    cells: Vec<Vec<gtk::Label>>,
    shape: (Vec<String>, usize),
    shown: Option<AbilityGrid>,
}

impl AbilityGridView {
    pub fn new() -> Self {
        let max_order = gtk::Label::builder().xalign(0.0).wrap(true).build();
        let grid = gtk::Grid::builder()
            .row_spacing(4)
            .column_spacing(6)
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid)
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.append(&max_order);
        root.append(&scroll);
        Self {
            root,
            max_order,
            grid,
            cells: Vec::new(),
            shape: (Vec::new(), 0),
            shown: None,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn show(&mut self, grid: Option<&AbilityGrid>) {
        if self.shown.as_ref() == grid {
            return;
        }
        let Some(grid) = grid else {
            self.root.set_visible(false);
            self.shown = None;
            return;
        };
        self.root.set_visible(true);
        let shape = (grid.rows.clone(), grid.points.len());
        if shape != self.shape {
            self.rebuild(&shape);
            self.shape = shape;
        }
        self.max_order
            .set_label(&crate::presentation::i18n::translate_with(
                "Ordem em que cada habilidade recebeu o último ponto nesta partida: {}",
                &[&grid.max_order],
            ));
        self.grid
            .update_property(&[gtk::accessible::Property::Description(&grid.summary)]);
        let states: Vec<Option<StepState>> = (0..grid.points.len())
            .map(|column| grid.points.get(column).map(|(_, state)| *state))
            .collect();
        for (row, texts) in grid_cells(grid).into_iter().enumerate() {
            for (column, text) in texts.into_iter().enumerate() {
                let cell = &self.cells[row][column];
                cell.set_label(text);
                let state = states[column].filter(|_| !text.is_empty());
                for (class, active) in [
                    ("success", state == Some(StepState::Done)),
                    ("accent", state == Some(StepState::Next)),
                    ("dim-label", state == Some(StepState::Pending)),
                ] {
                    if active {
                        cell.add_css_class(class);
                    } else {
                        cell.remove_css_class(class);
                    }
                }
            }
        }
        self.shown = Some(grid.clone());
    }

    /// Recria as células para outra forma de grade. Acontece ao trocar de
    /// build, não a cada evento do GSI.
    fn rebuild(&mut self, (rows, columns): &(Vec<String>, usize)) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        for column in 0..*columns {
            let header = gtk::Label::new(Some(&(column + 1).to_string()));
            header.add_css_class("dim-label");
            header.add_css_class("caption");
            self.grid.attach(&header, column as i32 + 1, 0, 1, 1);
        }
        self.cells = rows
            .iter()
            .enumerate()
            .map(|(row, name)| {
                let label = gtk::Label::builder().label(name).xalign(0.0).build();
                self.grid.attach(&label, 0, row as i32 + 1, 1, 1);
                (0..*columns)
                    .map(|column| {
                        let cell = gtk::Label::new(None);
                        cell.set_width_chars(2);
                        self.grid
                            .attach(&cell, column as i32 + 1, row as i32 + 1, 1, 1);
                        cell
                    })
                    .collect()
            })
            .collect();
    }
}

impl Default for AbilityGridView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/presentation/build_widgets_tests.rs"]
mod tests;
