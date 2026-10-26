//! Builds reais de vencedores de partidas públicas e cursor da coleta,
//! gravados em SQLite.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use serde_json::{Value, json};

use crate::application::ports::{PublicBuildStore, PublicWin, StorageError};
use crate::domain::builds::{PlayerBuild, Purchase};

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

impl PublicBuildStore for SqlitePublicBuildStore {
    fn save_match(&self, wins: &[PublicWin], cursor: u64) -> Result<(), StorageError> {
        let save_error = |sqlite_error| operation_error("gravar as builds públicas", sqlite_error);
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(save_error)?;
        for win in wins {
            let build = &win.build;
            let purchases = build.purchases.as_ref().map(|purchases| {
                Value::Array(
                    purchases
                        .iter()
                        .map(|purchase| json!({"item": purchase.item, "time": purchase.time}))
                        .collect(),
                )
                .to_string()
            });
            transaction
                .execute(
                    "INSERT OR IGNORE INTO public_wins
                     (match_id, hero_id, game_mode, patch_id, start_time, avg_rank_tier,
                      inventory, backpack, neutral, neutral_enhancement, purchases)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        to_sql_id(win.match_id),
                        win.hero_id,
                        win.game_mode,
                        win.patch_id,
                        win.start_time,
                        win.avg_rank_tier,
                        json!(build.inventory).to_string(),
                        json!(build.backpack).to_string(),
                        build.neutral,
                        build.neutral_enhancement,
                        purchases,
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

    fn recent_wins(
        &self,
        hero_id: u32,
        game_modes: &[u32],
        patch_id: u32,
        limit: usize,
    ) -> Result<Vec<PublicWin>, StorageError> {
        let read_error = |sqlite_error| operation_error("ler as builds públicas", sqlite_error);
        if game_modes.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; game_modes.len()].join(", ");
        // Linhas que não se leem como build são puladas, então a consulta
        // não usa LIMIT: o limite vale para as vitórias válidas.
        let sql = format!(
            "SELECT match_id, game_mode, start_time, avg_rank_tier, inventory, backpack,
                    neutral, neutral_enhancement, purchases
             FROM public_wins
             WHERE hero_id = ? AND patch_id = ? AND game_mode IN ({placeholders})
             ORDER BY start_time IS NULL, start_time DESC, match_id DESC"
        );
        let connection = self.connection();
        let mut statement = connection.prepare(&sql).map_err(read_error)?;
        let values = [i64::from(hero_id), i64::from(patch_id)]
            .into_iter()
            .chain(game_modes.iter().map(|mode| i64::from(*mode)));
        let mut rows = statement
            .query(params_from_iter(values))
            .map_err(read_error)?;
        let mut wins = Vec::new();
        while wins.len() < limit
            && let Some(row) = rows.next().map_err(read_error)?
        {
            let stored = StoredWin {
                match_id: row.get(0).map_err(read_error)?,
                game_mode: row.get(1).map_err(read_error)?,
                start_time: row.get(2).map_err(read_error)?,
                avg_rank_tier: row.get(3).map_err(read_error)?,
                inventory: row.get(4).map_err(read_error)?,
                backpack: row.get(5).map_err(read_error)?,
                neutral: row.get(6).map_err(read_error)?,
                neutral_enhancement: row.get(7).map_err(read_error)?,
                purchases: row.get(8).map_err(read_error)?,
            };
            // Linha que não se lê como build é ignorada, em vez de invalidar
            // todas as outras.
            if let Some(win) = stored.decode(hero_id, patch_id) {
                wins.push(win);
            }
        }
        Ok(wins)
    }

    fn purge_other_patches(&self, patch_id: u32) -> Result<usize, StorageError> {
        self.connection()
            .execute(
                "DELETE FROM public_wins WHERE patch_id <> ?1",
                params![patch_id],
            )
            .map_err(|sqlite_error| operation_error("limpar as builds públicas", sqlite_error))
    }
}

/// Colunas de uma vitória como estão no banco.
struct StoredWin {
    match_id: i64,
    game_mode: u32,
    start_time: Option<i64>,
    avg_rank_tier: Option<u32>,
    inventory: String,
    backpack: String,
    neutral: Option<String>,
    neutral_enhancement: Option<String>,
    purchases: Option<String>,
}

impl StoredWin {
    fn decode(self, hero_id: u32, patch_id: u32) -> Option<PublicWin> {
        let purchases = match self.purchases {
            Some(purchases) => Some(decode_purchases(&purchases)?),
            None => None,
        };
        Some(PublicWin {
            match_id: u64::try_from(self.match_id).ok()?,
            hero_id,
            game_mode: self.game_mode,
            patch_id,
            start_time: self.start_time,
            avg_rank_tier: self.avg_rank_tier,
            build: PlayerBuild {
                inventory: serde_json::from_str(&self.inventory).ok()?,
                backpack: serde_json::from_str(&self.backpack).ok()?,
                neutral: self.neutral,
                neutral_enhancement: self.neutral_enhancement,
                purchases,
            },
        })
    }
}

/// Lista de `{"item", "time"}`; qualquer registro fora desse formato
/// invalida a lista inteira, para não mostrar uma ordem de compra pela
/// metade.
fn decode_purchases(raw: &str) -> Option<Vec<Purchase>> {
    let entries: Vec<Value> = serde_json::from_str(raw).ok()?;
    entries
        .iter()
        .map(|entry| {
            Some(Purchase {
                item: entry.get("item")?.as_str()?.to_owned(),
                time: i32::try_from(entry.get("time")?.as_i64()?).ok()?,
            })
        })
        .collect()
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
