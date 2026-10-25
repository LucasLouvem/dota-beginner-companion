//! Builds de partidas públicas e cursor da coleta, gravados em SQLite.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::application::ports::{PublicBuild, PublicBuildStore, StorageError};
use crate::domain::builds::{BuildEvidence, BuildKey, MatchBuild};

use super::schema::{open_database, sqlite_kind};

/// Quanto uma gravação espera se outra conexão estiver gravando no banco.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds públicas em SQLite.
pub struct SqlitePublicBuildStore {
    connection: Mutex<Connection>,
}

impl SqlitePublicBuildStore {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let connection = open_database(path)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|sqlite_error| {
                operation_error("configurar as builds públicas", sqlite_error)
            })?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Uma gravação que falha desfaz a transação dela, então a conexão
    /// continua utilizável.
    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Nome da evidência gravado no banco.
fn evidence_name(evidence: BuildEvidence) -> &'static str {
    match evidence {
        BuildEvidence::OrderedPurchaseLog => "ordered_purchase_log",
        BuildEvidence::FinalInventoryUnordered => "final_inventory_unordered",
    }
}

fn evidence_from(name: &str) -> Option<BuildEvidence> {
    match name {
        "ordered_purchase_log" => Some(BuildEvidence::OrderedPurchaseLog),
        "final_inventory_unordered" => Some(BuildEvidence::FinalInventoryUnordered),
        _ => None,
    }
}

impl PublicBuildStore for SqlitePublicBuildStore {
    fn save_match(&self, builds: &[PublicBuild], cursor: u64) -> Result<(), StorageError> {
        let save_error = |sqlite_error| operation_error("gravar as builds públicas", sqlite_error);
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(save_error)?;
        for public in builds {
            let items = serde_json::to_string(&public.build.key.items)
                .expect("uma lista de textos sempre vira JSON");
            let times = public.build.times.as_ref().map(|times| {
                serde_json::to_string(times).expect("uma lista de números sempre vira JSON")
            });
            transaction
                .execute(
                    "INSERT OR IGNORE INTO public_builds
                     (match_id, hero_id, game_mode, patch_id, start_time, avg_rank_tier,
                      won, evidence, items, times)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        to_sql_id(public.match_id),
                        public.hero_id,
                        public.game_mode,
                        public.patch_id,
                        public.start_time,
                        public.avg_rank_tier,
                        public.won,
                        evidence_name(public.build.key.evidence),
                        items,
                        times,
                    ],
                )
                .map_err(save_error)?;
        }
        transaction
            .execute(
                "INSERT INTO collection_cursor (id, less_than_match_id) VALUES (1, ?1)
                 ON CONFLICT(id) DO UPDATE SET less_than_match_id = excluded.less_than_match_id",
                params![to_sql_id(cursor)],
            )
            .map_err(save_error)?;
        transaction.commit().map_err(save_error)
    }

    fn cursor(&self) -> Result<Option<u64>, StorageError> {
        self.connection()
            .query_row(
                "SELECT less_than_match_id FROM collection_cursor WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|cursor| cursor.and_then(|cursor| u64::try_from(cursor).ok()))
            .map_err(|sqlite_error| operation_error("ler o cursor da coleta", sqlite_error))
    }

    fn reset_cursor(&self) -> Result<(), StorageError> {
        self.connection()
            .execute("DELETE FROM collection_cursor", [])
            .map(|_| ())
            .map_err(|sqlite_error| operation_error("reiniciar o cursor da coleta", sqlite_error))
    }

    fn builds(
        &self,
        hero_id: u32,
        game_modes: &[u32],
        patch_id: u32,
    ) -> Result<Vec<(MatchBuild, bool)>, StorageError> {
        let read_error = |sqlite_error| operation_error("ler as builds públicas", sqlite_error);
        if game_modes.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; game_modes.len()].join(", ");
        let sql = format!(
            "SELECT evidence, items, times, won FROM public_builds
             WHERE hero_id = ? AND patch_id = ? AND game_mode IN ({placeholders})"
        );
        let connection = self.connection();
        let mut statement = connection.prepare(&sql).map_err(read_error)?;
        let values = [hero_id, patch_id]
            .into_iter()
            .chain(game_modes.iter().copied());
        let rows = statement
            .query_map(params_from_iter(values), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })
            .map_err(read_error)?;
        let mut builds = Vec::new();
        for row in rows {
            let (evidence, items, times, won) = row.map_err(read_error)?;
            // Linha que não se lê como build é ignorada, em vez de invalidar
            // todas as outras.
            let Some(build) = decode_build(&evidence, &items, times.as_deref()) else {
                continue;
            };
            builds.push((build, won));
        }
        Ok(builds)
    }

    fn purge_other_patches(&self, patch_id: u32) -> Result<usize, StorageError> {
        self.connection()
            .execute(
                "DELETE FROM public_builds WHERE patch_id <> ?1",
                params![patch_id],
            )
            .map_err(|sqlite_error| operation_error("limpar as builds públicas", sqlite_error))
    }
}

fn decode_build(evidence: &str, items: &str, times: Option<&str>) -> Option<MatchBuild> {
    Some(MatchBuild {
        key: BuildKey {
            evidence: evidence_from(evidence)?,
            items: serde_json::from_str(items).ok()?,
        },
        times: match times {
            Some(times) => Some(serde_json::from_str(times).ok()?),
            None => None,
        },
    })
}

/// IDs de partida cabem em `i64` (o tipo inteiro do SQLite) por muitas
/// ordens de grandeza; o limite só protege contra valor corrompido.
fn to_sql_id(match_id: u64) -> i64 {
    i64::try_from(match_id).unwrap_or(i64::MAX)
}

fn operation_error(operation: &'static str, sqlite_error: rusqlite::Error) -> StorageError {
    StorageError::Operation {
        operation,
        kind: sqlite_kind(&sqlite_error),
        source: Arc::new(sqlite_error),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/storage/public_builds_tests.rs"]
mod tests;
