//! Testes unitários de `src/presentation/build_widgets.rs`: só as funções
//! sem widgets.

use super::*;

fn chip(time: Option<&str>, state: Option<StepState>) -> ItemChip {
    ItemChip {
        item: "item_blink".into(),
        label: "Blink Dagger".into(),
        time: time.map(str::to_owned),
        state,
    }
}

#[test]
fn caption_joins_the_state_marker_and_the_time() {
    assert_eq!(
        chip_caption(&chip(Some("12:34"), Some(StepState::Done))),
        "✓ 12:34"
    );
    assert_eq!(chip_caption(&chip(None, Some(StepState::Next))), "→");
    assert_eq!(
        chip_caption(&chip(Some("antes do início"), None)),
        "antes do início"
    );
    assert_eq!(chip_caption(&chip(None, Some(StepState::Pending))), "");
}

#[test]
fn grid_cells_mark_only_the_row_of_each_point() {
    let grid = AbilityGrid {
        rows: vec!["Lightning Bolt".into(), "Arc Lightning".into()],
        points: vec![
            (0, StepState::Done),
            (1, StepState::Next),
            (0, StepState::Pending),
        ],
        max_order: String::new(),
        summary: String::new(),
    };

    assert_eq!(grid_cells(&grid), [vec!["✓", "", "●"], vec!["", "→", ""]]);
}

#[test]
fn grid_cells_ignore_a_point_outside_the_rows() {
    let grid = AbilityGrid {
        rows: vec!["Lightning Bolt".into()],
        points: vec![(5, StepState::Done)],
        max_order: String::new(),
        summary: String::new(),
    };
    assert_eq!(grid_cells(&grid), [vec![""]]);
}
