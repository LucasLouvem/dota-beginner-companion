//! Testes unitários de `src/presentation/card_grid.rs`: só as posições, sem
//! widgets.

use super::*;

#[test]
fn cards_fill_rows_from_left_to_right() {
    assert_eq!(placements(&[1, 1, 1], 3), [(0, 0, 1), (1, 0, 1), (2, 0, 1)]);
}

#[test]
fn the_last_card_of_a_row_takes_the_remaining_columns() {
    assert_eq!(placements(&[1, 1, 1], 2), [(0, 0, 1), (1, 0, 1), (0, 1, 2)]);
    assert_eq!(placements(&[1, 1], 3), [(0, 0, 1), (1, 0, 2)]);
    assert_eq!(placements(&[1], 3), [(0, 0, 3)]);
}

#[test]
fn a_wide_card_is_capped_to_the_columns() {
    assert_eq!(placements(&[2, 1, 1], 4), [(0, 0, 2), (2, 0, 1), (3, 0, 1)]);
    assert_eq!(placements(&[2, 1, 1], 2), [(0, 0, 2), (0, 1, 1), (1, 1, 1)]);
    assert_eq!(placements(&[2, 1, 1], 1), [(0, 0, 1), (0, 1, 1), (0, 2, 1)]);
}

#[test]
fn no_cards_and_zero_columns_are_harmless() {
    assert!(placements(&[], 3).is_empty());
    assert_eq!(placements(&[1, 1], 0), [(0, 0, 1), (0, 1, 1)]);
}

#[test]
fn each_layout_picks_its_columns() {
    let columns = Columns::new(3, 2);
    assert_eq!(columns.for_layout(Layout::Wide), 3);
    assert_eq!(columns.for_layout(Layout::Medium), 2);
    assert_eq!(Columns::new(0, 0).for_layout(Layout::Wide), 1);
}
