//! Testes unitários de `src/application/use_cases/recommendation_request.rs`.

use super::*;

fn key(hero_id: u32, mode: GameMode) -> RecommendationKey {
    RecommendationKey {
        hero_id,
        mode,
        patch_id: 60,
    }
}

#[test]
fn late_answer_for_a_previous_hero_is_rejected() {
    let mut tracker = RequestTracker::default();
    let hero_a = tracker.start(key(22, GameMode::AllPick)).unwrap();
    let hero_b = tracker.start(key(49, GameMode::AllPick)).unwrap();

    // A resposta de B chega primeiro e é aceita; a de A chega depois.
    assert!(tracker.accepts(&hero_b));
    assert!(!tracker.accepts(&hero_a));
}

#[test]
fn changing_the_mode_rejects_the_answer_for_the_previous_mode() {
    let mut tracker = RequestTracker::default();
    let all_pick = tracker.start(key(22, GameMode::AllPick)).unwrap();
    let turbo = tracker.start(key(22, GameMode::Turbo)).unwrap();

    assert!(!tracker.accepts(&all_pick));
    assert!(tracker.accepts(&turbo));
}

#[test]
fn same_key_again_does_not_start_another_request() {
    let mut tracker = RequestTracker::default();
    let first = tracker.start(key(22, GameMode::AllPick)).unwrap();

    assert_eq!(tracker.start(key(22, GameMode::AllPick)), None);
    assert!(tracker.accepts(&first));
}

#[test]
fn going_back_to_a_previous_key_does_not_revive_its_old_answer() {
    let mut tracker = RequestTracker::default();
    let old_a = tracker.start(key(22, GameMode::AllPick)).unwrap();
    tracker.start(key(49, GameMode::AllPick)).unwrap();
    let new_a = tracker.start(key(22, GameMode::AllPick)).unwrap();

    assert!(
        !tracker.accepts(&old_a),
        "a consulta antiga de A foi substituída"
    );
    assert!(tracker.accepts(&new_a));
}

#[test]
fn restart_replaces_the_current_request_with_the_same_key() {
    let mut tracker = RequestTracker::default();
    let first = tracker.start(key(22, GameMode::AllPick)).unwrap();

    let refreshed = tracker.restart(key(22, GameMode::AllPick));

    assert!(!tracker.accepts(&first));
    assert!(tracker.accepts(&refreshed));
}

#[test]
fn clearing_rejects_pending_answers() {
    let mut tracker = RequestTracker::default();
    let pending = tracker.start(key(22, GameMode::AllPick)).unwrap();

    tracker.clear();

    assert!(!tracker.accepts(&pending));
}
