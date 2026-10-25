//! Testes unitários de `src/storage/call_budget.rs`, com bancos em diretórios
//! temporários.

use super::*;
use crate::application::ports::{BudgetExhausted, BudgetLimit};
use std::time::UNIX_EPOCH;

/// 2026-10-25T12:00:00Z
const DAY_ONE: u64 = 1_792_929_600;
const DAY: u64 = 86_400;

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn small_limits() -> BudgetLimits {
    BudgetLimits {
        daily_total: 3,
        daily_collection: 2,
        monthly_total: 100,
        monthly_collection: 100,
    }
}

fn exhausted(category: CallCategory, limit: BudgetLimit) -> BudgetExhausted {
    BudgetExhausted { category, limit }
}

fn temp_budget(limits: BudgetLimits) -> (tempfile::TempDir, SqliteCallBudget) {
    let temp_dir = tempfile::tempdir().unwrap();
    let budget =
        SqliteCallBudget::open(&temp_dir.path().join("companion.sqlite3"), limits).unwrap();
    (temp_dir, budget)
}

fn refusal(result: Result<(), BudgetError>) -> BudgetExhausted {
    match result {
        Err(BudgetError::Exhausted(exhausted)) => exhausted,
        other => panic!("esperava orçamento esgotado: {other:?}"),
    }
}

#[test]
fn reservations_within_the_limit_are_granted_and_then_refused() {
    let (_temp_dir, budget) = temp_budget(small_limits());

    assert!(
        budget
            .reserve(CallCategory::Collection, at(DAY_ONE))
            .is_ok()
    );
    assert!(
        budget
            .reserve(CallCategory::Collection, at(DAY_ONE))
            .is_ok()
    );

    assert_eq!(
        refusal(budget.reserve(CallCategory::Collection, at(DAY_ONE))),
        exhausted(CallCategory::Collection, BudgetLimit::DailyCollection)
    );
}

#[test]
fn exhausted_collection_leaves_the_reserved_part_for_the_screens() {
    let (_temp_dir, budget) = temp_budget(small_limits());
    budget
        .reserve(CallCategory::Collection, at(DAY_ONE))
        .unwrap();
    budget
        .reserve(CallCategory::Collection, at(DAY_ONE))
        .unwrap();

    assert!(budget.reserve(CallCategory::OnDemand, at(DAY_ONE)).is_ok());
    assert_eq!(
        refusal(budget.reserve(CallCategory::OnDemand, at(DAY_ONE))),
        exhausted(CallCategory::OnDemand, BudgetLimit::DailyTotal)
    );
}

#[test]
fn new_utc_day_renews_the_daily_limit_but_keeps_the_month() {
    let limits = BudgetLimits {
        monthly_total: 4,
        ..small_limits()
    };
    let (_temp_dir, budget) = temp_budget(limits);
    for _ in 0..3 {
        budget.reserve(CallCategory::OnDemand, at(DAY_ONE)).unwrap();
    }

    assert!(
        budget
            .reserve(CallCategory::OnDemand, at(DAY_ONE + DAY))
            .is_ok()
    );
    assert_eq!(
        refusal(budget.reserve(CallCategory::OnDemand, at(DAY_ONE + DAY))),
        exhausted(CallCategory::OnDemand, BudgetLimit::MonthlyTotal)
    );
}

#[test]
fn new_month_renews_the_monthly_limit() {
    let limits = BudgetLimits {
        monthly_total: 1,
        ..small_limits()
    };
    let (_temp_dir, budget) = temp_budget(limits);
    budget.reserve(CallCategory::OnDemand, at(DAY_ONE)).unwrap();
    assert!(budget.reserve(CallCategory::OnDemand, at(DAY_ONE)).is_err());

    // 2026-11-01T00:00:00Z
    assert!(
        budget
            .reserve(CallCategory::OnDemand, at(1_793_491_200))
            .is_ok()
    );
}

#[test]
fn reservations_persist_after_reopening() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    let budget = SqliteCallBudget::open(&path, small_limits()).unwrap();
    for _ in 0..3 {
        budget.reserve(CallCategory::OnDemand, at(DAY_ONE)).unwrap();
    }
    drop(budget);

    let reopened = SqliteCallBudget::open(&path, small_limits()).unwrap();

    assert_eq!(
        refusal(reopened.reserve(CallCategory::OnDemand, at(DAY_ONE))),
        exhausted(CallCategory::OnDemand, BudgetLimit::DailyTotal)
    );
}

#[test]
fn concurrent_reservations_from_two_connections_never_exceed_the_limit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    let limits = BudgetLimits {
        daily_total: 25,
        daily_collection: 25,
        monthly_total: 1000,
        monthly_collection: 1000,
    };
    let budgets = [
        Arc::new(SqliteCallBudget::open(&path, limits).unwrap()),
        Arc::new(SqliteCallBudget::open(&path, limits).unwrap()),
    ];

    let threads: Vec<_> = (0..8)
        .map(|index| {
            let budget = Arc::clone(&budgets[index % 2]);
            std::thread::spawn(move || {
                (0..10)
                    .filter(|_| budget.reserve(CallCategory::OnDemand, at(DAY_ONE)).is_ok())
                    .count()
            })
        })
        .collect();
    let granted: usize = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .sum();

    assert_eq!(granted, 25);
}

#[test]
fn failure_to_open_is_a_storage_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("companion.sqlite3");
    std::fs::write(
        &path,
        "isto não é um banco SQLite, só texto comum com tamanho suficiente",
    )
    .unwrap();

    let Err(storage_error) = SqliteCallBudget::open(&path, small_limits()) else {
        panic!("esperava falha ao abrir");
    };
    assert!(
        matches!(storage_error, StorageError::Open { .. }),
        "{storage_error:?}"
    );
}
