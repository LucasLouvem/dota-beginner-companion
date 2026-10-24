//! Casos de uso e portas. Depende só de `domain`; os adaptadores entram por
//! traits definidas em `ports`.

pub mod core;
pub mod event_bus;
pub mod ports;
pub mod state;
pub mod use_cases;
