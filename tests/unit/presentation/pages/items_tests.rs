//! Testes unitários de `src/presentation/pages/items.rs` (conteúdo da aba,
//! sem widgets).

use super::*;
use crate::application::use_cases::fallback::Sourced;
use crate::application::use_cases::pro_builds::ProBuilds;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::domain::builds::{BuildKey, BuildRanking};
use crate::domain::hero_catalog::{AttackType, HeroInfo, PrimaryAttribute};
use crate::domain::item_catalog::ItemInfo;
use crate::domain::match_data::ItemPopularity;
use crate::domain::patch::Patch;
use std::collections::BTreeMap;
use std::time::UNIX_EPOCH;

fn heroes() -> HeroCatalog {
    HeroCatalog::new(vec![HeroInfo {
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![],
        abilities: vec![],
    }])
}

fn items() -> ItemCatalog {
    let item = |id: u32, name: &str, display_name: &str| ItemInfo {
        id,
        name: name.into(),
        display_name: display_name.into(),
        cost: Some(4000),
        components: vec![],
    };
    ItemCatalog::new(vec![
        item(1, "item_black_king_bar", "Black King Bar"),
        item(2, "item_desolator", "Desolator"),
        item(3, "item_satanic", "Satanic"),
        item(4, "item_branches", "Iron Branch"),
    ])
}

fn key(mode: GameMode) -> RecommendationKey {
    RecommendationKey {
        hero_id: 22,
        mode,
        patch_id: 60,
    }
}

fn ordered_build(wins: usize, matches: usize) -> RankedBuild {
    RankedBuild {
        key: BuildKey {
            evidence: BuildEvidence::OrderedPurchaseLog,
            items: vec![
                "item_black_king_bar".into(),
                "item_desolator".into(),
                "item_satanic".into(),
            ],
        },
        matches,
        wins,
        average_times: Some(vec![900, 1300, 1800]),
    }
}

fn unordered_build(wins: usize, matches: usize) -> RankedBuild {
    RankedBuild {
        key: BuildKey {
            evidence: BuildEvidence::FinalInventoryUnordered,
            items: vec![
                "item_black_king_bar".into(),
                "item_satanic".into(),
                "item_unknown".into(),
            ],
        },
        matches,
        wins,
        average_times: None,
    }
}

fn pro(builds: Vec<RankedBuild>, valid_matches: usize, origin: Option<DataOrigin>) -> ProBuilds {
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

fn recommendations(
    mode: GameMode,
    pro_builds: ProBuilds,
    public: Vec<RankedBuild>,
) -> Recommendations {
    Recommendations {
        key: key(mode),
        patch: Patch {
            id: 60,
            name: "7.41".into(),
        },
        pro: pro_builds,
        public: BuildRanking {
            analyzed: public.iter().map(|build| build.matches).sum(),
            builds: public,
        },
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

fn content(status: &RecommendationStatus) -> ItemsContent {
    items_content(status, &heroes(), &items(), &Redactor::default())
}

fn titles(rows: &[RowContent]) -> Vec<&str> {
    rows.iter().map(|row| row.title.as_str()).collect()
}

#[test]
fn without_sources_or_hero_the_header_says_why() {
    assert_eq!(
        titles(&content(&RecommendationStatus::Unavailable).header),
        ["Recomendações indisponíveis"]
    );
    assert_eq!(
        titles(&content(&RecommendationStatus::Idle).header),
        ["Nenhum herói para recomendar"]
    );
    let loading = content(&RecommendationStatus::Loading {
        key: key(GameMode::Turbo),
    });
    assert_eq!(
        loading.header,
        vec![RowContent::new(
            Level::Info,
            "Carregando recomendações de Zeus",
            "Turbo"
        )]
    );
    assert!(loading.pro.is_empty());
}

#[test]
fn turbo_warns_that_professional_matches_are_not_turbo() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::Turbo,
        pro(vec![ordered_build(9, 12)], 30, Some(DataOrigin::Live)),
        vec![],
    ))));

    assert_eq!(ready.header[0].title, "Zeus · Turbo · patch 7.41");
    assert_eq!(ready.pro[0].title, "Partidas profissionais não são turbo");
    assert_eq!(ready.pro[0].level, Level::Warning);
    assert_eq!(ready.pro[1].title, "Black King Bar → Desolator → Satanic");
}

#[test]
fn all_pick_has_no_turbo_warning_and_bots_says_its_data_is_all_pick() {
    let all_pick = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(vec![ordered_build(9, 12)], 30, Some(DataOrigin::Live)),
        vec![],
    ))));
    let bots = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::Bots,
        pro(vec![], 30, Some(DataOrigin::Live)),
        vec![],
    ))));

    assert_eq!(
        all_pick.pro[0].title,
        "Black King Bar → Desolator → Satanic"
    );
    assert_eq!(all_pick.notes.len(), 1);
    assert_eq!(
        all_pick.notes[0].title,
        "Dados públicos de todas as faixas de rank; a recomendação não considera posição."
    );
    assert_eq!(
        bots.notes[1].title,
        "Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores."
    );
}

#[test]
fn sample_below_minimum_is_shown_with_the_analyzed_count() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(vec![], 30, Some(DataOrigin::Live)),
        vec![],
    ))));

    assert_eq!(
        ready.pro[0],
        RowContent::new(
            Level::Info,
            "Amostra insuficiente",
            "Nenhuma build com pelo menos 10 partidas entre 30 partidas profissionais válidas."
        )
    );
    assert_eq!(ready.public[0].title, "Amostra insuficiente");
}

#[test]
fn stale_professional_data_is_marked_as_old() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(vec![ordered_build(9, 12)], 30, Some(DataOrigin::Stale)),
        vec![],
    ))));

    let source = ready.pro.last().unwrap();
    assert_eq!(source.level, Level::Warning);
    assert!(source.subtitle.starts_with("Dados antigos"), "{source:?}");
}

#[test]
fn public_builds_keep_the_ranking_order_and_say_when_there_is_no_order() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::Turbo,
        pro(vec![], 0, None),
        vec![unordered_build(8, 10), ordered_build(6, 10)],
    ))));

    assert_eq!(
        titles(&ready.public[..2]),
        [
            "Black King Bar + Satanic + item_unknown",
            "Black King Bar → Desolator → Satanic"
        ]
    );
    assert_eq!(
        ready.public[0].subtitle,
        "Taxa de vitória observada: 80% em 10 partidas · itens finais, sem ordem de compra"
    );
}

#[test]
fn ordered_build_row_shows_rate_sample_and_average_times() {
    assert_eq!(
        build_row(&ordered_build(9, 12), &items()),
        RowContent::new(
            Level::Info,
            "Black King Bar → Desolator → Satanic",
            "Taxa de vitória observada: 75% em 12 partidas · tempo médio: 15:00, 21:40, 30:00"
        )
    );
}

#[test]
fn popularity_lists_the_most_bought_items_by_phase_with_the_source() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(vec![], 0, None),
        vec![],
    ))));

    assert_eq!(ready.popularity[0].title, "Início: Iron Branch, Satanic");
    assert_eq!(ready.popularity[1].level, Level::Warning, "dados antigos");
}

#[test]
fn win_percent_rounds_without_floating_point() {
    assert_eq!(win_percent(2, 3), 67);
    assert_eq!(win_percent(1, 2), 50);
    assert_eq!(win_percent(1, 3), 33);
    assert_eq!(win_percent(0, 0), 0);
}

#[test]
fn purchase_before_the_start_has_no_clock() {
    assert_eq!(purchase_time(-89), "antes do início");
    assert_eq!(purchase_time(1300), "21:40");
}

#[test]
fn every_phase_has_a_label() {
    let labels: Vec<String> = [
        PopularityPhase::Start,
        PopularityPhase::Early,
        PopularityPhase::Mid,
        PopularityPhase::Late,
    ]
    .into_iter()
    .map(phase_label)
    .collect();
    assert_eq!(
        labels,
        [
            "Início",
            "Até 10 minutos",
            "De 10 a 25 minutos",
            "Depois de 25 minutos"
        ]
    );
}
