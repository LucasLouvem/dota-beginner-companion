//! Testes unitários de `src/storage/public_builds.rs`, com bancos em
//! diretórios temporários.

use super::*;

fn ordered(items: &[&str], times: &[i32]) -> MatchBuild {
    MatchBuild {
        key: BuildKey {
            evidence: BuildEvidence::OrderedPurchaseLog,
            items: items.iter().map(|item| (*item).into()).collect(),
        },
        times: Some(times.to_vec()),
    }
}

fn unordered(items: &[&str]) -> MatchBuild {
    MatchBuild {
        key: BuildKey {
            evidence: BuildEvidence::FinalInventoryUnordered,
            items: items.iter().map(|item| (*item).into()).collect(),
        },
        times: None,
    }
}

fn public(
    match_id: u64,
    hero_id: u32,
    game_mode: u32,
    patch_id: u32,
    build: MatchBuild,
) -> PublicBuild {
    PublicBuild {
        match_id,
        hero_id,
        game_mode,
        patch_id,
        start_time: Some(1_792_970_329),
        avg_rank_tier: Some(51),
        won: true,
        build,
    }
}

fn temp_store() -> (tempfile::TempDir, SqlitePublicBuildStore) {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = SqlitePublicBuildStore::open(&temp_dir.path().join("companion.sqlite3")).unwrap();
    (temp_dir, store)
}

#[test]
fn saved_builds_are_read_back_with_evidence_items_times_and_result() {
    let (_temp_dir, store) = temp_store();
    let build = ordered(&["item_a", "item_b", "item_c"], &[900, 1300, 1800]);
    let lost = PublicBuild {
        won: false,
        ..public(2, 22, 23, 60, unordered(&["item_x", "item_y", "item_z"]))
    };

    store
        .save_match(&[public(1, 22, 23, 60, build.clone())], 1)
        .unwrap();
    store.save_match(&[lost], 2).unwrap();

    let mut builds = store.builds(22, &[23], 60).unwrap();
    builds.sort_by_key(|(build, _)| build.key.evidence);
    assert_eq!(
        builds,
        vec![
            (build, true),
            (unordered(&["item_x", "item_y", "item_z"]), false),
        ]
    );
}

#[test]
fn builds_are_filtered_by_hero_mode_and_patch() {
    let (_temp_dir, store) = temp_store();
    let build = unordered(&["item_a", "item_b", "item_c"]);
    store
        .save_match(
            &[
                public(1, 22, 1, 60, build.clone()),
                public(1, 49, 1, 60, build.clone()),
            ],
            1,
        )
        .unwrap();
    store
        .save_match(&[public(2, 22, 23, 60, build.clone())], 2)
        .unwrap();
    store
        .save_match(&[public(3, 22, 22, 59, build.clone())], 3)
        .unwrap();

    assert_eq!(store.builds(22, &[1, 22], 60).unwrap().len(), 1);
    assert_eq!(store.builds(22, &[23], 60).unwrap().len(), 1);
    assert_eq!(store.builds(22, &[1, 22], 59).unwrap().len(), 1);
    assert!(store.builds(22, &[], 60).unwrap().is_empty());
}

#[test]
fn the_same_hero_in_the_same_match_is_saved_once() {
    let (_temp_dir, store) = temp_store();
    let build = unordered(&["item_a", "item_b", "item_c"]);

    store
        .save_match(&[public(1, 22, 1, 60, build.clone())], 1)
        .unwrap();
    store.save_match(&[public(1, 22, 1, 60, build)], 1).unwrap();

    assert_eq!(store.builds(22, &[1], 60).unwrap().len(), 1);
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
fn builds_and_cursor_persist_after_reopening() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    SqlitePublicBuildStore::open(&path)
        .unwrap()
        .save_match(
            &[public(
                1,
                22,
                23,
                60,
                unordered(&["item_a", "item_b", "item_c"]),
            )],
            1,
        )
        .unwrap();

    let reopened = SqlitePublicBuildStore::open(&path).unwrap();

    assert_eq!(reopened.cursor().unwrap(), Some(1));
    assert_eq!(reopened.builds(22, &[23], 60).unwrap().len(), 1);
}

#[test]
fn failed_save_keeps_neither_the_builds_nor_the_new_cursor() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    let store = SqlitePublicBuildStore::open(&path).unwrap();
    store.save_match(&[], 100).unwrap();
    // Um gatilho que recusa o cursor novo faz a gravação falhar depois de
    // inserir as builds.
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_cursor BEFORE UPDATE ON collection_cursor
             BEGIN SELECT RAISE(ABORT, 'cursor recusado'); END;",
        )
        .unwrap();

    let result = store.save_match(
        &[public(
            1,
            22,
            23,
            60,
            unordered(&["item_a", "item_b", "item_c"]),
        )],
        50,
    );

    assert!(
        matches!(result, Err(StorageError::Operation { .. })),
        "{result:?}"
    );
    assert_eq!(store.cursor().unwrap(), Some(100));
    assert!(store.builds(22, &[23], 60).unwrap().is_empty());
}

#[test]
fn purge_removes_builds_of_other_patches() {
    let (_temp_dir, store) = temp_store();
    let build = unordered(&["item_a", "item_b", "item_c"]);
    store
        .save_match(&[public(1, 22, 23, 59, build.clone())], 1)
        .unwrap();
    store
        .save_match(&[public(2, 22, 23, 60, build)], 2)
        .unwrap();

    assert_eq!(store.purge_other_patches(60).unwrap(), 1);

    assert!(store.builds(22, &[23], 59).unwrap().is_empty());
    assert_eq!(store.builds(22, &[23], 60).unwrap().len(), 1);
}

#[test]
fn row_that_does_not_read_as_a_build_is_skipped() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    let store = SqlitePublicBuildStore::open(&path).unwrap();
    store
        .save_match(
            &[public(
                1,
                22,
                23,
                60,
                unordered(&["item_a", "item_b", "item_c"]),
            )],
            1,
        )
        .unwrap();
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO public_builds VALUES (2, 22, 23, 60, NULL, NULL, 1, 'outra', '[]', NULL)",
            [],
        )
        .unwrap();

    assert_eq!(store.builds(22, &[23], 60).unwrap().len(), 1);
}

#[test]
fn build_of_a_match_without_start_time_is_saved() {
    let (_temp_dir, store) = temp_store();
    let without_time = PublicBuild {
        start_time: None,
        avg_rank_tier: None,
        ..public(1, 22, 23, 60, unordered(&["item_a", "item_b", "item_c"]))
    };

    store.save_match(&[without_time], 1).unwrap();

    assert_eq!(store.builds(22, &[23], 60).unwrap().len(), 1);
}
