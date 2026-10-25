//! Providers de dados externos. Implementam os ports de consulta de
//! `application::ports`; a rede roda sempre em tarefas Tokio.

pub mod http_json;
