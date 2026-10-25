//! Política do aplicativo para as chamadas ao OpenDota: limites por dia e por
//! mês, separados entre consultas das telas e coleta em segundo plano. Os
//! valores são escolhas conservadoras do aplicativo, não limites oficiais do
//! OpenDota.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::{BudgetExhausted, BudgetLimit, CallCategory};

/// Limites do orçamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetLimits {
    /// Chamadas por dia, somando as duas categorias.
    pub daily_total: u32,
    /// Parte do dia que a coleta pode usar; o resto fica para as telas.
    pub daily_collection: u32,
    pub monthly_total: u32,
    pub monthly_collection: u32,
}

impl Default for BudgetLimits {
    /// 1.000 por dia, com até 700 para a coleta (300 sempre livres para as
    /// telas); 30.000 por mês, com até 21.000 para a coleta.
    fn default() -> Self {
        Self {
            daily_total: 1000,
            daily_collection: 700,
            monthly_total: 30_000,
            monthly_collection: 21_000,
        }
    }
}

/// Chamadas já reservadas no dia e no mês atuais.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BudgetUsage {
    pub day_total: u32,
    pub day_collection: u32,
    pub month_total: u32,
    pub month_collection: u32,
}

/// Confere se cabe mais uma chamada da categoria. A coleta respeita os
/// próprios limites e os totais; as telas só os totais, então a coleta nunca
/// ocupa a parte reservada para elas.
pub fn check_reservation(
    category: CallCategory,
    usage: &BudgetUsage,
    limits: &BudgetLimits,
) -> Result<(), BudgetExhausted> {
    let exhausted = |limit| Err(BudgetExhausted { category, limit });
    if usage.day_total >= limits.daily_total {
        return exhausted(BudgetLimit::DailyTotal);
    }
    if usage.month_total >= limits.monthly_total {
        return exhausted(BudgetLimit::MonthlyTotal);
    }
    if category == CallCategory::Collection {
        if usage.day_collection >= limits.daily_collection {
            return exhausted(BudgetLimit::DailyCollection);
        }
        if usage.month_collection >= limits.monthly_collection {
            return exhausted(BudgetLimit::MonthlyCollection);
        }
    }
    Ok(())
}

/// Dia (`2026-10-25`) e mês (`2026-10`) em UTC, as chaves do orçamento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetPeriod {
    pub day: String,
    pub month: String,
}

/// Período de `now` em UTC. Horários anteriores a 1970 contam como 1970-01-01.
pub fn budget_period(now: SystemTime) -> BudgetPeriod {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    BudgetPeriod {
        day: format!("{year:04}-{month:02}-{day:02}"),
        month: format!("{year:04}-{month:02}"),
    }
}

/// Data do calendário gregoriano para `days` dias desde 1970-01-01. É o
/// algoritmo `civil_from_days` de Howard Hinnant ("chrono-Compatible Low-Level
/// Date Algorithms"), que conta em eras de 400 anos a partir de março.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "../../../tests/unit/application/use_cases/call_budget_tests.rs"]
mod tests;
