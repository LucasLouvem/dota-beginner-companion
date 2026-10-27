//! Grade de cards que muda o número de colunas conforme a largura da página.
//! Os cards são criados uma vez; só a posição na grade é refeita, quando as
//! colunas mudam ou um card aparece ou some.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use relm4::gtk::{self, prelude::*};

/// Composição da página conforme a largura disponível.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Lista de builds ao lado do conteúdo.
    Wide,
    /// Lista ao lado, grupos com menos colunas.
    Medium,
}

/// Colunas de um grupo em cada composição.
#[derive(Debug, Clone, Copy)]
pub struct Columns {
    pub wide: u32,
    pub medium: u32,
}

impl Columns {
    pub const fn new(wide: u32, medium: u32) -> Self {
        Columns { wide, medium }
    }

    pub fn for_layout(self, layout: Layout) -> u32 {
        match layout {
            Layout::Wide => self.wide,
            Layout::Medium => self.medium,
        }
        .max(1)
    }
}

struct Slot {
    widget: gtk::Widget,
    /// Colunas que o card ocupa em cada composição, no máximo as da grade.
    span: Columns,
    visible: bool,
}

struct Inner {
    grid: gtk::Grid,
    columns: Columns,
    current: Cell<u32>,
    layout: Cell<Layout>,
    slots: RefCell<Vec<Slot>>,
}

/// Grade de colunas iguais. Clonar devolve outra referência à mesma grade.
#[derive(Clone)]
pub struct CardGrid(Rc<Inner>);

impl CardGrid {
    pub fn new(gap: i32, columns: Columns) -> Self {
        let grid = gtk::Grid::builder()
            .column_homogeneous(true)
            .column_spacing(gap)
            .row_spacing(gap)
            .hexpand(true)
            .build();
        CardGrid(Rc::new(Inner {
            grid,
            columns,
            current: Cell::new(columns.wide.max(1)),
            layout: Cell::new(Layout::Wide),
            slots: RefCell::new(Vec::new()),
        }))
    }

    pub fn widget(&self) -> &gtk::Grid {
        &self.0.grid
    }

    /// Acrescenta um card, visível, ocupando `span` colunas.
    pub fn push(&self, widget: &impl IsA<gtk::Widget>, span: u32) {
        self.push_spans(widget, Columns::new(span, span));
    }

    /// Como `push`, com uma largura para cada composição: o mesmo card pode
    /// ocupar 3 de 4 colunas na larga e 2 de 3 na média.
    pub fn push_spans(&self, widget: &impl IsA<gtk::Widget>, span: Columns) {
        self.0.slots.borrow_mut().push(Slot {
            widget: widget.clone().upcast(),
            span,
            visible: true,
        });
        self.relayout();
    }

    /// Remove os cards a partir da posição `len`.
    pub fn truncate(&self, len: usize) {
        let removed: Vec<Slot> = {
            let mut slots = self.0.slots.borrow_mut();
            if slots.len() <= len {
                return;
            }
            slots.drain(len..).collect()
        };
        for slot in removed {
            if slot.widget.parent().is_some() {
                self.0.grid.remove(&slot.widget);
            }
        }
        self.relayout();
    }

    /// Mostra ou esconde o card `index`, refazendo as posições só se mudou.
    pub fn set_visible(&self, index: usize, visible: bool) {
        let changed = {
            let mut slots = self.0.slots.borrow_mut();
            match slots.get_mut(index) {
                Some(slot) if slot.visible != visible => {
                    slot.visible = visible;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.relayout();
        }
    }

    /// Ajusta as colunas à composição da página.
    pub fn set_layout(&self, layout: Layout) {
        let columns = self.0.columns.for_layout(layout);
        let previous = self.0.layout.replace(layout);
        if self.0.current.replace(columns) != columns || previous != layout {
            self.relayout();
        }
    }

    /// Reposiciona os cards visíveis, da esquerda para a direita. O último
    /// de cada linha ocupa o que sobrar dela, para não deixar buraco.
    fn relayout(&self) {
        let grid = &self.0.grid;
        let columns = self.0.current.get();
        let slots = self.0.slots.borrow();
        for slot in slots.iter() {
            if slot.widget.parent().is_some() {
                grid.remove(&slot.widget);
            }
        }
        let visible: Vec<&Slot> = slots.iter().filter(|slot| slot.visible).collect();
        let layout = self.0.layout.get();
        let spans: Vec<u32> = visible
            .iter()
            .map(|slot| slot.span.for_layout(layout))
            .collect();
        for (slot, (column, row, span)) in visible.iter().zip(placements(&spans, columns)) {
            grid.attach(&slot.widget, column as i32, row as i32, span as i32, 1);
        }
    }
}

/// Posições (coluna, linha, largura) que `relayout` dá a cada card visível.
/// Existe separada dos widgets para poder ser testada.
pub fn placements(spans: &[u32], columns: u32) -> Vec<(u32, u32, u32)> {
    let columns = columns.max(1);
    let (mut column, mut row) = (0, 0);
    let mut placed = Vec::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        let mut span = (*span).max(1).min(columns);
        if column + span > columns {
            column = 0;
            row += 1;
        }
        let next_fits = spans
            .get(index + 1)
            .is_some_and(|next| column + span + (*next).max(1).min(columns) <= columns);
        if !next_fits {
            span = columns - column;
        }
        placed.push((column, row, span));
        column += span;
        if column >= columns {
            column = 0;
            row += 1;
        }
    }
    placed
}

#[cfg(test)]
#[path = "../../tests/unit/presentation/card_grid_tests.rs"]
mod tests;
