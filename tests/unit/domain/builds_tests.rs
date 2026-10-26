//! Testes unitários de `src/domain/builds.rs`. Os dados de exemplo são
//! escritos à mão.

use super::*;

fn buy(item: &str, time: i32) -> Purchase {
    Purchase {
        item: item.into(),
        time,
    }
}

#[test]
fn purchases_follow_time_order_and_keep_repeated_and_pre_game_ones() {
    let ordered = in_purchase_order(vec![
        buy("item_blink", 600),
        buy("item_tango", -89),
        buy("item_ward_observer", 600),
        buy("item_tango", 300),
    ]);

    assert_eq!(
        ordered,
        [
            buy("item_tango", -89),
            buy("item_tango", 300),
            buy("item_blink", 600),
            buy("item_ward_observer", 600),
        ],
        "mesmo horário mantém a ordem da fonte"
    );
}
