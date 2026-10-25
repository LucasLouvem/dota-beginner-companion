//! Orçamento de chamadas ao OpenDota gravado em SQLite.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::application::ports::{BudgetError, CallBudget, CallCategory, StorageError};
use crate::application::use_cases::call_budget::{
    BudgetLimits, BudgetPeriod, BudgetUsage, budget_period, check_reservation,
};

use super::schema::{open_database, sqlite_kind};

/// Quanto uma reserva espera se outra conexão estiver gravando no banco.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Orçamento em SQLite. Cada reserva confere e incrementa os contadores numa
/// transação `IMMEDIATE`, que trava a escrita desde o início: duas reservas
/// simultâneas, mesmo de conexões diferentes, nunca passam do limite.
pub struct SqliteCallBudget {
    connection: Mutex<Connection>,
    limits: BudgetLimits,
}

impl SqliteCallBudget {
    pub fn open(path: &Path, limits: BudgetLimits) -> Result<Self, StorageError> {
        let connection = open_database(path)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|sqlite_error| operation_error("configurar o orçamento", sqlite_error))?;
        Ok(Self {
            connection: Mutex::new(connection),
            limits,
        })
    }

    /// Uma falha no meio de outra reserva desfaz a transação dela, então a
    /// conexão continua utilizável.
    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl CallBudget for SqliteCallBudget {
    fn reserve(&self, category: CallCategory, now: SystemTime) -> Result<(), BudgetError> {
        let period = budget_period(now);
        let mut connection = self.connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|sqlite_error| operation_error("reservar uma chamada", sqlite_error))?;
        let usage = read_usage(&transaction, &period)
            .map_err(|sqlite_error| operation_error("ler o orçamento", sqlite_error))?;
        check_reservation(category, &usage, &self.limits)?;
        for (period_name, period_key) in [("day", &period.day), ("month", &period.month)] {
            transaction
                .execute(
                    "INSERT INTO call_budget (category, period, period_key, used)
                     VALUES (?1, ?2, ?3, 1)
                     ON CONFLICT(category, period, period_key) DO UPDATE SET used = used + 1",
                    params![category.as_str(), period_name, period_key],
                )
                .map_err(|sqlite_error| operation_error("gravar o orçamento", sqlite_error))?;
        }
        transaction
            .commit()
            .map_err(|sqlite_error| operation_error("gravar o orçamento", sqlite_error))?;
        Ok(())
    }
}

fn read_usage(
    transaction: &Transaction<'_>,
    period: &BudgetPeriod,
) -> rusqlite::Result<BudgetUsage> {
    let used = |category: Option<CallCategory>, period_name: &str, period_key: &str| {
        transaction
            .query_row(
                "SELECT SUM(used) FROM call_budget
                 WHERE period = ?1 AND period_key = ?2 AND (?3 IS NULL OR category = ?3)",
                params![period_name, period_key, category.map(CallCategory::as_str)],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .map(|sum| u32::try_from(sum.flatten().unwrap_or(0)).unwrap_or(u32::MAX))
    };
    Ok(BudgetUsage {
        day_total: used(None, "day", &period.day)?,
        day_collection: used(Some(CallCategory::Collection), "day", &period.day)?,
        month_total: used(None, "month", &period.month)?,
        month_collection: used(Some(CallCategory::Collection), "month", &period.month)?,
    })
}

fn operation_error(operation: &'static str, sqlite_error: rusqlite::Error) -> StorageError {
    StorageError::Operation {
        operation,
        kind: sqlite_kind(&sqlite_error),
        source: Arc::new(sqlite_error),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/storage/call_budget_tests.rs"]
mod tests;
