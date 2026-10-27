//! Testes unitários de `src/presentation/build_widgets.rs`: só as funções
//! sem widgets.

use super::*;

fn chip(time: Option<i32>, state: Option<StepState>) -> ItemChip {
    ItemChip {
        item: "item_blink".into(),
        label: "Blink Dagger".into(),
        time,
        state,
    }
}

#[test]
fn caption_joins_the_state_marker_and_the_time() {
    assert_eq!(
        chip_caption(&chip(Some(754), Some(StepState::Done))),
        "✓ 12:34"
    );
    assert_eq!(
        chip_caption(&chip(None, Some(StepState::Next))),
        "→ próximo"
    );
    assert_eq!(chip_caption(&chip(Some(-89), None)), "antes do início");
    assert_eq!(chip_caption(&chip(None, Some(StepState::Pending))), "");
}

#[test]
fn grid_cells_mark_only_the_row_of_each_point() {
    let grid = AbilityGrid {
        rows: vec!["Lightning Bolt".into(), "Arc Lightning".into()],
        images: vec![None, None],
        points: vec![
            (0, StepState::Done),
            (1, StepState::Next),
            (0, StepState::Pending),
        ],
        summary: String::new(),
    };

    assert_eq!(grid_cells(&grid), [vec!["✓", "", "●"], vec!["", "→", ""]]);
}

#[test]
fn grid_cells_ignore_a_point_outside_the_rows() {
    let grid = AbilityGrid {
        rows: vec!["Lightning Bolt".into()],
        images: vec![None],
        points: vec![(5, StepState::Done)],
        summary: String::new(),
    };
    assert_eq!(grid_cells(&grid), [vec![""]]);
}
