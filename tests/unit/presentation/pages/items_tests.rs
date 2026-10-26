//! Testes unitários de `src/presentation/pages/items.rs` (conteúdo da aba,
//! sem widgets).

use super::*;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::domain::hero_catalog::{AttackType, HeroInfo, PrimaryAttribute};
use crate::domain::item_catalog::ItemInfo;
use crate::test_support::{pro_builds as pro, pro_win, public_win, recommendations, zeus_build};
use std::sync::Arc;

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
        item(5, "item_blink", "Blink Dagger"),
        item(6, "item_smoke_of_deceit", "Smoke of Deceit"),
        item(7, "item_partisans_brand", "Partisan's Brand"),
        item(8, "item_enhancement_greedy", "Greedy"),
        item(9, "item_tango", "Tango"),
    ])
}

fn key(mode: GameMode) -> RecommendationKey {
    RecommendationKey {
        hero_id: 22,
        mode,
        patch_id: 60,
    }
}

fn one_pro_win() -> Vec<ProWin> {
    vec![pro_win(1, zeus_build(), None)]
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
        pro(one_pro_win(), 30, Some(DataOrigin::Live)),
        vec![],
    ))));

    assert_eq!(ready.header[0].title, "Zeus · Turbo · patch 7.41");
    assert_eq!(ready.pro[0].title, "Partidas profissionais não são turbo");
    assert_eq!(ready.pro[0].level, Level::Warning);
    assert_eq!(ready.pro[1].title, "Vitória profissional: Liga Teste");
    assert_eq!(
        ready.pro[2].title,
        "Inventário final: Blink Dagger, Black King Bar"
    );
}

#[test]
fn all_pick_has_no_turbo_warning_and_bots_says_its_data_is_all_pick() {
    let all_pick = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(one_pro_win(), 30, Some(DataOrigin::Live)),
        vec![],
    ))));
    let bots = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::Bots,
        pro(vec![], 30, Some(DataOrigin::Live)),
        vec![],
    ))));

    assert_eq!(all_pick.pro[0].title, "Vitória profissional: Liga Teste");
    assert_eq!(all_pick.notes.len(), 1);
    assert_eq!(
        all_pick.notes[0].title,
        "As vitórias públicas são de todas as faixas de rank e de qualquer posição."
    );
    assert_eq!(
        bots.notes[1].title,
        "Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores."
    );
}

#[test]
fn without_wins_each_block_says_none_was_found() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(vec![], 30, Some(DataOrigin::Live)),
        vec![],
    ))));

    assert_eq!(
        ready.pro[0],
        RowContent::new(
            Level::Info,
            "Nenhuma vitória recente encontrada",
            "Entre as 30 partidas de liga recentes examinadas; derrotas, outros patches e partidas sem build ficam de fora."
        )
    );
    assert_eq!(ready.public[0].title, "Nenhuma vitória recente encontrada");
}

#[test]
fn stale_professional_data_is_marked_as_old() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::AllPick,
        pro(one_pro_win(), 30, Some(DataOrigin::Stale)),
        vec![],
    ))));

    let source = ready.pro.last().unwrap();
    assert_eq!(source.level, Level::Warning);
    assert!(source.subtitle.starts_with("Dados antigos"), "{source:?}");
}

#[test]
fn public_wins_show_the_real_build_of_each_match_and_how_many_were_found() {
    let ready = content(&RecommendationStatus::Ready(Arc::new(recommendations(
        GameMode::Turbo,
        pro(vec![], 0, None),
        vec![public_win(2, zeus_build()), public_win(1, zeus_build())],
    ))));

    assert_eq!(ready.public[0].title, "Vitória em partida pública");
    assert!(
        ready.public[0].subtitle.ends_with("rank médio Lenda 1"),
        "{:?}",
        ready.public[0]
    );
    assert_eq!(
        ready.public[1].title,
        "Inventário final: Blink Dagger, Black King Bar"
    );
    assert_eq!(ready.public[3].title, "Vitória em partida pública");
    assert_eq!(
        ready.public[6],
        RowContent::new(
            Level::Info,
            "2 vitórias recentes encontradas",
            "A coleta em segundo plano ainda não gravou mais vitórias do herói neste modo e patch."
        )
    );
    assert!(
        ready.public[7]
            .title
            .starts_with("Fonte: partidas públicas")
    );
}

#[test]
fn win_rows_show_inventory_backpack_neutral_and_every_purchase_with_its_time() {
    let rows = win_rows("Partida", "detalhe", &zeus_build(), &items());

    assert_eq!(
        rows,
        [
            RowContent::new(Level::Info, "Partida", "detalhe"),
            RowContent::new(
                Level::Info,
                "Inventário final: Blink Dagger, Black King Bar",
                "Mochila: Smoke of Deceit · Neutro: Partisan's Brand + Greedy"
            ),
            RowContent::new(
                Level::Info,
                "Ordem de compra",
                "Tango (antes do início) → Blink Dagger (12:34)"
            ),
        ]
    );
}

#[test]
fn win_rows_without_purchases_or_extra_slots_say_so() {
    let build = PlayerBuild {
        inventory: vec!["item_unknown".into()],
        ..PlayerBuild::default()
    };

    let rows = win_rows("Partida", "", &build, &items());

    assert_eq!(rows[1].title, "Inventário final: item_unknown");
    assert_eq!(rows[1].subtitle, "Mochila vazia · Sem item neutro");
    assert_eq!(rows[2].title, "Sem ordem de compra");
}

#[test]
fn found_row_only_appears_when_fewer_wins_than_shown() {
    assert_eq!(found_row(3, 3, "x"), None);
    assert_eq!(
        found_row(0, 3, "x").unwrap().title,
        "Nenhuma vitória recente encontrada"
    );
    assert_eq!(
        found_row(1, 3, "x").unwrap().title,
        "1 vitória recente encontrada"
    );
}

#[test]
fn rank_label_reads_medal_and_stars() {
    assert_eq!(rank_label(51).as_deref(), Some("Lenda 1"));
    assert_eq!(rank_label(10).as_deref(), Some("Arauto"));
    assert_eq!(rank_label(80).as_deref(), Some("Imortal"));
    assert_eq!(rank_label(0), None);
    assert_eq!(rank_label(95), None);
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
