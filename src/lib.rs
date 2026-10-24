//! Camadas do aplicativo, compostas em `main.rs` e usadas pelos testes de
//! integração em `tests/`.

pub mod application;
pub mod domain;
pub mod gsi;
pub mod infrastructure;
pub mod presentation;
pub mod storage;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;
