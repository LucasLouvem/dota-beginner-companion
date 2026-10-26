//! Testes unitários de `src/storage/public_builds.rs`, com bancos em
//! diretórios temporários.

use super::*;
use crate::application::ports::StorageError;

fn full_build() -> PlayerBuild {
    PlayerBuild {
        inventory: vec!["item_blink".into(), "item_black_king_bar".into()],
        backpack: vec!["item_smoke_of_deceit".into()],
        neutral: Some("item_partisans_brand".into()),
        neutral_enhancement: Some("item_enhancement_greedy".into()),
        purchases: Some(vec![
            Purchase {
                item: "item_tango".into(),
                time: -89,
            },
            Purchase {
                item: "item_tango".into(),
                time: 300,
            },
            Purchase {
                item: "item_blink".into(),
                time: 754,
            },
        ]),
    }
}

fn inventory_only() -> PlayerBuild {
    PlayerBuild {
        inventory: vec!["item_satanic".into()],
        ..PlayerBuild::default()
    }
}

fn win(match_id: u64, hero_id: u32, game_mode: u32, patch_id: u32) -> PublicWin {
    PublicWin {
        match_id,
        hero_id,
        game_mode,
        patch_id,
        // Mais recente quanto maior o ID, para a ordem do teste ser clara.
        start_time: Some(1_792_970_000 + match_id as i64),
        avg_rank_tier: Some(51),
        build: inventory_only(),
    }
}

fn temp_store() -> (tempfile::TempDir, SqlitePublicBuildStore) {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = SqlitePublicBuildStore::open(&temp_dir.path().join("companion.sqlite3")).unwrap();
    (temp_dir, store)
}

fn match_ids(wins: &[PublicWin]) -> Vec<u64> {
    wins.iter().map(|win| win.match_id).collect()
}

#[test]
fn saved_win_is_read_back_with_the_whole_build() {
    let (_temp_dir, store) = temp_store();
    let full = PublicWin {
        build: full_build(),
        ..win(1, 22, 23, 60)
    };
    let without_extras = PublicWin {
        start_time: None,
        avg_rank_tier: None,
        ..win(2, 22, 23, 60)
    };

    store.save_match(std::slice::from_ref(&full), 1).unwrap();
    store
        .save_match(std::slice::from_ref(&without_extras), 2)
        .unwrap();

    assert_eq!(
        store.recent_wins(22, &[23], 60, 5).unwrap(),
        [full, without_extras],
        "partida sem horário fica por último"
    );
}

#[test]
fn recent_wins_come_newest_first_up_to_the_limit() {
    let (_temp_dir, store) = temp_store();
    for match_id in [3, 1, 4, 2] {
        store
            .save_match(&[win(match_id, 22, 1, 60)], match_id)
            .unwrap();
    }
    let same_time = PublicWin {
        start_time: Some(1_792_970_004),
        ..win(5, 22, 22, 60)
    };
    store.save_match(&[same_time], 5).unwrap();

    let wins = store.recent_wins(22, &[1, 22], 60, 3).unwrap();

    assert_eq!(
        match_ids(&wins),
        [5, 4, 3],
        "empate no horário: maior ID antes"
    );
}

#[test]
fn recent_wins_are_filtered_by_hero_mode_and_patch() {
    let (_temp_dir, store) = temp_store();
    store
        .save_match(&[win(1, 22, 1, 60), win(1, 49, 1, 60)], 1)
        .unwrap();
    store.save_match(&[win(2, 22, 23, 60)], 2).unwrap();
    store.save_match(&[win(3, 22, 22, 59)], 3).unwrap();

    assert_eq!(
        match_ids(&store.recent_wins(22, &[1, 22], 60, 5).unwrap()),
        [1]
    );
    assert_eq!(
        match_ids(&store.recent_wins(22, &[23], 60, 5).unwrap()),
        [2]
    );
    assert_eq!(
        match_ids(&store.recent_wins(22, &[1, 22], 59, 5).unwrap()),
        [3]
    );
    assert!(store.recent_wins(22, &[], 60, 5).unwrap().is_empty());
    assert!(store.recent_wins(22, &[1], 60, 0).unwrap().is_empty());
}

#[test]
fn the_same_hero_in_the_same_match_is_saved_once() {
    let (_temp_dir, store) = temp_store();
    let first = win(1, 22, 1, 60);
    let repeated = PublicWin {
        build: full_build(),
        ..win(1, 22, 1, 60)
    };

    store.save_match(std::slice::from_ref(&first), 1).unwrap();
    store.save_match(&[repeated], 1).unwrap();

    assert_eq!(store.recent_wins(22, &[1], 60, 5).unwrap(), [first]);
}

#[test]
fn cursor_moves_with_each_saved_match_and_can_be_reset() {
    let (_temp_dir, store) = temp_store();
    assert_eq!(store.cursor().unwrap(), None);

    store.save_match(&[], 9_062_205_691).unwrap();
    assert_eq!(store.cursor().unwrap(), Some(9_062_205_691));
    store.save_match(&[], 9_062_205_000).unwrap();
    assert_eq!(store.cursor().unwrap(), Some(9_062_205_000));

    store.reset_cursor().unwrap();
    assert_eq!(store.cursor().unwrap(), None);
}

#[test]
fn wins_and_cursor_persist_after_reopening() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    SqlitePublicBuildStore::open(&path)
        .unwrap()
        .save_match(&[win(1, 22, 23, 60)], 1)
        .unwrap();

    let reopened = SqlitePublicBuildStore::open(&path).unwrap();

    assert_eq!(reopened.cursor().unwrap(), Some(1));
    assert_eq!(reopened.recent_wins(22, &[23], 60, 5).unwrap().len(), 1);
}

#[test]
fn failed_save_keeps_neither_the_wins_nor_the_new_cursor() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    let store = SqlitePublicBuildStore::open(&path).unwrap();
    store.save_match(&[], 100).unwrap();
    // Um gatilho que recusa o cursor novo faz a gravação falhar depois de
    // inserir as vitórias.
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_cursor BEFORE UPDATE ON collection_cursor
             BEGIN SELECT RAISE(ABORT, 'cursor recusado'); END;",
        )
        .unwrap();

    let result = store.save_match(&[win(1, 22, 23, 60)], 50);

    assert!(
        matches!(result, Err(StorageError::Operation { .. })),
        "{result:?}"
    );
    assert_eq!(store.cursor().unwrap(), Some(100));
    assert!(store.recent_wins(22, &[23], 60, 5).unwrap().is_empty());
}

#[test]
fn purge_removes_wins_of_other_patches() {
    let (_temp_dir, store) = temp_store();
    store.save_match(&[win(1, 22, 23, 59)], 1).unwrap();
    store.save_match(&[win(2, 22, 23, 60)], 2).unwrap();

    assert_eq!(store.purge_other_patches(60).unwrap(), 1);

    assert!(store.recent_wins(22, &[23], 59, 5).unwrap().is_empty());
    assert_eq!(store.recent_wins(22, &[23], 60, 5).unwrap().len(), 1);
}

#[test]
fn rows_that_do_not_read_as_a_build_are_skipped_without_using_the_limit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    let store = SqlitePublicBuildStore::open(&path).unwrap();
    store.save_match(&[win(1, 22, 23, 60)], 1).unwrap();
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "INSERT INTO public_wins VALUES
                (2, 22, 23, 60, 1792970002, NULL, 'outra', '[]', NULL, NULL, NULL);
             INSERT INTO public_wins VALUES
                (3, 22, 23, 60, 1792970003, NULL, '[]', '[]', NULL, NULL, '[{\"item\": 1}]');",
        )
        .unwrap();

    assert_eq!(
        match_ids(&store.recent_wins(22, &[23], 60, 1).unwrap()),
        [1]
    );
}
