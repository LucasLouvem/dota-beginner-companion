//! Providers de dados externos. Implementam os ports de consulta de
//! `application::ports`; a rede roda sempre em tarefas Tokio.

pub mod http_json;
pub mod opendota;
pub mod steam_cdn;
