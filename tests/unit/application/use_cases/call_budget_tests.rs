//! Testes unitários de `src/application/use_cases/call_budget.rs`.

use super::*;
use std::time::Duration;

fn limits() -> BudgetLimits {
    BudgetLimits {
        daily_total: 10,
        daily_collection: 7,
        monthly_total: 100,
        monthly_collection: 70,
    }
}

fn usage(day_total: u32, day_collection: u32) -> BudgetUsage {
    BudgetUsage {
        day_total,
        day_collection,
        month_total: day_total,
        month_collection: day_collection,
    }
}

#[test]
fn default_limits_keep_300_daily_calls_for_the_screens() {
    let defaults = BudgetLimits::default();
    assert_eq!(defaults.daily_total, 1000);
    assert_eq!(defaults.daily_collection, 700);
    assert_eq!(defaults.daily_total - defaults.daily_collection, 300);
}

#[test]
fn reservation_within_the_limits_is_granted() {
    assert_eq!(
        check_reservation(CallCategory::Collection, &usage(6, 6), &limits()),
        Ok(())
    );
    assert_eq!(
        check_reservation(CallCategory::OnDemand, &usage(9, 7), &limits()),
        Ok(())
    );
}

#[test]
fn collection_stops_at_its_daily_share() {
    assert_eq!(
        check_reservation(CallCategory::Collection, &usage(7, 7), &limits()),
        Err(BudgetExhausted {
            category: CallCategory::Collection,
            limit: BudgetLimit::DailyCollection,
        })
    );
}

#[test]
fn exhausted_collection_does_not_block_the_screens() {
    assert_eq!(
        check_reservation(CallCategory::OnDemand, &usage(7, 7), &limits()),
        Ok(())
    );
}

#[test]
fn screens_stop_at_the_daily_total() {
    assert_eq!(
        check_reservation(CallCategory::OnDemand, &usage(10, 7), &limits()),
        Err(BudgetExhausted {
            category: CallCategory::OnDemand,
            limit: BudgetLimit::DailyTotal,
        })
    );
}

#[test]
fn monthly_limits_apply_even_on_a_new_day() {
    let month_spent = BudgetUsage {
        day_total: 0,
        day_collection: 0,
        month_total: 100,
        month_collection: 70,
    };
    assert_eq!(
        check_reservation(CallCategory::OnDemand, &month_spent, &limits()),
        Err(BudgetExhausted {
            category: CallCategory::OnDemand,
            limit: BudgetLimit::MonthlyTotal,
        })
    );
    let collection_month_spent = BudgetUsage {
        month_total: 80,
        ..month_spent
    };
    assert_eq!(
        check_reservation(CallCategory::Collection, &collection_month_spent, &limits()),
        Err(BudgetExhausted {
            category: CallCategory::Collection,
            limit: BudgetLimit::MonthlyCollection,
        })
    );
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

#[test]
fn period_is_the_utc_day_and_month() {
    // 2026-10-25T12:00:00Z
    assert_eq!(
        budget_period(at(1_792_929_600)),
        BudgetPeriod {
            day: "2026-10-25".into(),
            month: "2026-10".into(),
        }
    );
}

#[test]
fn period_turns_at_utc_midnight_of_month_and_year() {
    // 2026-10-31T23:59:59Z e 2026-11-01T00:00:00Z
    assert_eq!(budget_period(at(1_793_491_199)).day, "2026-10-31");
    assert_eq!(budget_period(at(1_793_491_200)).month, "2026-11");
    // 2027-12-31T23:59:59Z e 2028-01-01T00:00:00Z
    assert_eq!(budget_period(at(1_830_297_599)).day, "2027-12-31");
    assert_eq!(budget_period(at(1_830_297_600)).day, "2028-01-01");
}

#[test]
fn leap_day_and_epoch_are_dated_correctly() {
    // 2028-02-29T00:00:00Z
    assert_eq!(budget_period(at(1_835_395_200)).day, "2028-02-29");
    assert_eq!(budget_period(UNIX_EPOCH).day, "1970-01-01");
}
