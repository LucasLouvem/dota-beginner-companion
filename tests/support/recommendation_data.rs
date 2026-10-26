//! Recomendações prontas para os testes das abas de recomendação.

use std::collections::BTreeMap;
use std::time::UNIX_EPOCH;

use crate::application::ports::PublicWin;
use crate::application::use_cases::fallback::{DataOrigin, Sourced};
use crate::application::use_cases::pro_builds::ProBuilds;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::application::use_cases::recommendations::Recommendations;
use crate::domain::builds::{BuildRanking, PlayerBuild, Purchase, RankedBuild};
use crate::domain::game::GameMode;
use crate::domain::match_data::{ItemPopularity, PopularityPhase};
use crate::domain::patch::Patch;

/// Resultado profissional com as builds, `valid_matches` partidas válidas e a
/// origem da lista de candidatas.
pub fn pro_builds(
    builds: Vec<RankedBuild>,
    valid_matches: usize,
    origin: Option<DataOrigin>,
) -> ProBuilds {
    ProBuilds {
        ranking: BuildRanking {
            builds,
            analyzed: valid_matches,
        },
        ability_order: None,
        recent_matches: vec![],
        examined: valid_matches,
        valid_matches,
        discarded: BTreeMap::new(),
        stopped_by: None,
        candidates_origin: origin,
        error: None,
        calls: 0,
    }
}

/// Recomendações do Zeus (herói 22) no patch 7.41, consultadas em 1970, com
/// a popularidade de início antiga (Iron Branch e Satanic).
pub fn recommendations(
    mode: GameMode,
    pro: ProBuilds,
    public_wins: Vec<PublicWin>,
) -> Recommendations {
    Recommendations {
        key: RecommendationKey {
            hero_id: 22,
            mode,
            patch_id: 60,
        },
        patch: Patch {
            id: 60,
            name: "7.41".into(),
        },
        pro,
        public_wins,
        public_error: None,
        popularity: Sourced {
            value: Some(ItemPopularity {
                phases: vec![(
                    PopularityPhase::Start,
                    vec![("item_branches".into(), 189), ("item_satanic".into(), 3)],
                )],
            }),
            origin: Some(DataOrigin::Stale),
            fetched_at: Some(UNIX_EPOCH),
            error: None,
        },
        loaded_at: UNIX_EPOCH,
    }
}

/// Build real do Zeus: inventário com espaço da Blink Dagger, mochila,
/// neutro com aprimoramento e compras desde antes do início.
pub fn zeus_build() -> PlayerBuild {
    PlayerBuild {
        inventory: vec!["item_blink".into(), "item_black_king_bar".into()],
        backpack: vec!["item_smoke_of_deceit".into()],
        neutral: Some("item_partisans_brand".into()),
        neutral_enhancement: Some("item_enhancement_greedy".into()),
        purchases: Some(vec![
            Purchase {
                item: "item_tango".into(),
                time: -89,
            },
            Purchase {
                item: "item_blink".into(),
                time: 754,
            },
        ]),
    }
}

/// Vitória pública do Zeus no Turbo, em 25/10/2026, com rank médio Lenda 1.
pub fn public_win(match_id: u64, build: PlayerBuild) -> PublicWin {
    PublicWin {
        match_id,
        hero_id: 22,
        game_mode: 23,
        patch_id: 60,
        start_time: Some(1_792_970_000),
        avg_rank_tier: Some(51),
        build,
    }
}
