//! Orçamento que sempre concede.

use std::time::SystemTime;

use crate::application::ports::{BudgetError, CallBudget, CallCategory};

/// Concede toda reserva; para testes que não tratam do orçamento.
pub struct UnlimitedBudget;

impl CallBudget for UnlimitedBudget {
    fn reserve(&self, _category: CallCategory, _now: SystemTime) -> Result<(), BudgetError> {
        Ok(())
    }
}
