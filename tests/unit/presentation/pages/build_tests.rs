//! Testes unitários de `src/presentation/pages/build.rs` (conteúdo da página
//! da build, sem widgets).

use super::*;
use crate::application::ports::ProviderError;
use crate::application::use_cases::pro_builds::ProWin;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::domain::builds::PlayerBuild;
use crate::domain::game::{AbilityState, CarriedItem};
use crate::domain::hero_catalog::{AbilityInfo, AttackType, HeroInfo, PrimaryAttribute};
use crate::domain::item_catalog::ItemInfo;
use crate::test_support::{pro_builds, pro_win, public_win, recommendations, zeus_build};
use std::collections::HashMap;
use std::sync::Arc;

fn heroes() -> HeroCatalog {
    HeroCatalog::new(vec![HeroInfo {
        image_path: None,
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![],
        abilities: vec![
            AbilityInfo {
                image_path: None,
                name: "zuus_arc_lightning".into(),
                display_name: "Arc Lightning".into(),
            },
            AbilityInfo {
                image_path: None,
                name: "zuus_lightning_bolt".into(),
                display_name: "Lightning Bolt".into(),
            },
        ],
    }])
}

fn items() -> ItemCatalog {
    let item = |name: &str, display_name: &str, components: &[&str]| ItemInfo {
        id: 0,
        name: name.into(),
        display_name: display_name.into(),
        cost: None,
        components: components
            .iter()
            .map(|component| (*component).into())
            .collect(),
        image_path: None,
    };
    ItemCatalog::new(vec![
        item("item_blink", "Blink Dagger", &[]),
        item(
            "item_black_king_bar",
            "Black King Bar",
            &["item_ogre_axe", "item_mithril_hammer"],
        ),
        item("item_tango", "Tango", &[]),
        item("item_smoke_of_deceit", "Smoke of Deceit", &[]),
        item("item_partisans_brand", "Partisan's Brand", &[]),
        item("item_enhancement_greedy", "Greedy", &[]),
    ])
}

fn abilities() -> AbilityIdCatalog {
    AbilityIdCatalog::new(HashMap::from([
        (5110, "zuus_arc_lightning".to_owned()),
        (5111, "zuus_lightning_bolt".to_owned()),
        (6000, "special_bonus_spell_amplify_8".to_owned()),
    ]))
}

fn detail(
    recommendations: &Recommendations,
    id: &BuildId,
    game: Option<&GameSnapshot>,
) -> Option<BuildDetail> {
    let (heroes, items, abilities) = (heroes(), items(), abilities());
    build_detail(
        recommendations,
        id,
        game,
        &BuildCatalogs {
            heroes: &heroes,
            items: &items,
            abilities: &abilities,
        },
    )
}

const PRO: BuildId = BuildId {
    source: BuildSource::Pro,
    match_id: 1,
};
const PUBLIC: BuildId = BuildId {
    source: BuildSource::Public,
    match_id: 2,
};

fn listed(mode: GameMode, pro: Vec<ProWin>) -> Recommendations {
    recommendations(
        mode,
        pro_builds(pro, 8, Some(DataOrigin::Live)),
        vec![public_win(
            2,
            PlayerBuild {
                purchases: None,
                ..zeus_build()
            },
        )],
    )
}

fn ready(recommendations: Recommendations) -> RecommendationStatus {
    RecommendationStatus::Ready(Arc::new(recommendations))
}

fn titles(rows: &[RowContent]) -> Vec<&str> {
    rows.iter().map(|row| row.title.as_str()).collect()
}

fn sidebar_of(status: &RecommendationStatus) -> Sidebar {
    sidebar(status)
}

fn header_of(status: &RecommendationStatus) -> HeaderContent {
    header_content(status, &heroes(), &Redactor::default())
}

/// Avisos do cabeçalho.
fn header(status: &RecommendationStatus) -> Vec<RowContent> {
    header_of(status).notices
}

#[test]
fn header_shows_hero_mode_patch_as_title_and_counts_as_details() {
    let header = header_of(&ready(listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), None)],
    )));

    assert_eq!(header.title, "Zeus · Escolha Livre · patch 7.41");
    assert!(
        header
            .details
            .starts_with("1 vitórias profissionais e 1 públicas · fonte: OpenDota, 8 partidas de liga recentes examinadas · Consultado às"),
        "{}",
        header.details
    );
    assert_eq!(titles(&header.notices), ["Poucas vitórias encontradas"]);
}

#[test]
fn header_before_ready_has_a_plain_title_and_the_state_as_notice() {
    let header = header_of(&RecommendationStatus::Idle);
    assert_eq!(header.title, "Build");
    assert!(header.details.is_empty());
    assert_eq!(titles(&header.notices), ["Nenhum herói para recomendar"]);
}

#[test]
fn header_warns_about_bots_but_not_about_turbo() {
    let turbo = header(&ready(listed(GameMode::Turbo, vec![])));
    let bots = header(&ready(listed(GameMode::Bots, vec![])));
    let all_pick = header(&ready(listed(GameMode::AllPick, vec![])));

    assert!(
        !titles(&turbo).contains(&"Partidas profissionais não são turbo"),
        "o aviso de Turbo é da build profissional, não da página"
    );
    assert!(titles(&bots).contains(
        &"Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores."
    ));
    assert!(
        !titles(&all_pick)
            .iter()
            .chain(&titles(&turbo))
            .any(|title| title.starts_with("Modo ainda")),
        "o modo não informado aparece no card do modo, não como aviso"
    );
}

#[test]
fn header_marks_old_data_and_shows_provider_errors() {
    let mut stale = listed(GameMode::AllPick, vec![]);
    stale.pro.candidates_origin = Some(DataOrigin::Stale);
    stale.pro.error = Some(ProviderError::RateLimited {
        path: "/heroes/22/matches".into(),
    });

    let rows = header(&ready(stale));

    let old = rows
        .iter()
        .find(|row| row.title.starts_with("Dados profissionais antigos"))
        .expect("aviso de dados antigos");
    assert_eq!(old.level, Level::Warning);
    assert!(
        rows.iter()
            .any(|row| row.title == "Dados do OpenDota indisponíveis"),
        "{rows:?}"
    );
}

#[test]
fn header_before_ready_explains_the_state() {
    assert_eq!(
        titles(&header(&RecommendationStatus::Idle)),
        ["Nenhum herói para recomendar"]
    );
    assert_eq!(
        titles(&header(&RecommendationStatus::Unavailable)),
        ["Recomendações indisponíveis"]
    );
}

#[test]
fn sidebar_lists_professional_then_public_wins_without_item_thumbnails() {
    let list = sidebar_of(&ready(listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), None)],
    )));

    assert_eq!(list.pro[0].id, PRO);
    assert_eq!(list.pro[0].title, "Liga Teste");
    assert_eq!(list.pro[0].subtitle, "40:00");
    assert_eq!(list.public[0].id, PUBLIC);
    assert_eq!(list.public[0].title, "Rank médio Lenda 1");
    assert_eq!(list.empty, None);
    assert_eq!(list.entries().count(), 2);
}

#[test]
fn sidebar_without_wins_or_hero_explains_why() {
    let no_wins = sidebar_of(&ready(recommendations(
        GameMode::AllPick,
        pro_builds(vec![], 12, None),
        vec![],
    )));
    let no_hero = sidebar_of(&RecommendationStatus::Idle);

    assert_eq!(
        no_wins.empty.unwrap().title,
        "Nenhuma vitória recente encontrada"
    );
    assert_eq!(no_hero.empty.unwrap().title, "Nenhuma build para escolher");
}

#[test]
fn selection_never_changes_the_header_or_the_sidebar() {
    // Cabeçalho e lista não recebem a seleção: escolher outra build só muda
    // o conteúdo da build.
    let status = ready(listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), None)],
    ));
    let recommendations = match &status {
        RecommendationStatus::Ready(recommendations) => Arc::clone(recommendations),
        _ => unreachable!(),
    };

    let pro = detail(&recommendations, &PRO, None).unwrap();
    let public = detail(&recommendations, &PUBLIC, None).unwrap();

    assert_ne!(pro, public);
}

#[test]
fn unknown_build_has_no_detail() {
    let recommendations = listed(GameMode::AllPick, vec![]);
    let gone = BuildId {
        source: BuildSource::Pro,
        match_id: 999,
    };
    assert_eq!(detail(&recommendations, &gone, None), None);
}

fn carrying(items: &[&str]) -> GameSnapshot {
    GameSnapshot {
        inventory: Some(
            items
                .iter()
                .map(|name| CarriedItem {
                    slot: "slot0".into(),
                    name: (*name).into(),
                })
                .collect(),
        ),
        ..GameSnapshot::menu()
    }
}

#[test]
fn analyzed_build_shows_starting_items_order_with_progress_and_windows() {
    let recommendations = listed(GameMode::AllPick, vec![pro_win(1, zeus_build(), None)]);
    let game = carrying(&["item_blink"]);

    let build = detail(&recommendations, &PRO, Some(&game)).unwrap();

    assert!(build.notes.is_empty());
    assert_eq!(build.starting[0].label, "Tango");
    assert_eq!(
        build.starting[0].time, None,
        "o título do card já diz que é antes do início"
    );
    assert_eq!(build.starting[0].accessible_label(), "Tango");
    let order: Vec<(&str, Option<StepState>)> = build
        .order
        .iter()
        .map(|chip| (chip.label.as_str(), chip.state))
        .collect();
    assert_eq!(
        order,
        [
            ("Blink Dagger", Some(StepState::Done)),
            ("Black King Bar", Some(StepState::Next)),
            ("Smoke of Deceit", Some(StepState::Pending)),
        ],
        "Blink comprada aos 12:34; BKB e Smoke sem compra registrada vão para o fim, na ordem dos espaços (inventário, depois mochila)"
    );
    assert_eq!(build.neutral[0].label, "Partisan's Brand");
    assert_eq!(build.enhancement[0].label, "Greedy");
    assert_eq!(build.backpack[0].label, "Smoke of Deceit");
}

#[test]
fn components_of_an_assembled_item_count_as_bought() {
    let recommendations = listed(GameMode::AllPick, vec![pro_win(1, zeus_build(), None)]);
    let game = carrying(&["item_black_king_bar"]);

    let build = detail(&recommendations, &PRO, Some(&game)).unwrap();

    let bkb = build
        .order
        .iter()
        .find(|chip| chip.item == "item_black_king_bar")
        .unwrap();
    assert_eq!(bkb.state, Some(StepState::Done));
    assert_eq!(
        build.order[0].state,
        Some(StepState::Next),
        "Blink ainda não"
    );
}

#[test]
fn public_build_without_purchase_log_says_the_order_is_by_slot() {
    let recommendations = listed(GameMode::AllPick, vec![]);

    let build = detail(&recommendations, &PUBLIC, None).unwrap();

    assert_eq!(titles(&build.notes), ["Partida sem registro de compras"]);
    assert!(build.starting.is_empty());
    assert_eq!(build.order.len(), 3);
    assert_eq!(build.abilities, None);
    assert_eq!(
        build.abilities_note.unwrap().title,
        "Sem ordem de habilidades"
    );
}

#[test]
fn only_the_professional_build_in_turbo_warns_that_it_is_not_turbo() {
    let recommendations = listed(GameMode::Turbo, vec![pro_win(1, zeus_build(), None)]);

    let pro = detail(&recommendations, &PRO, None).unwrap();
    let public = detail(&recommendations, &PUBLIC, None).unwrap();

    assert_eq!(pro.notes[0].title, "Partidas profissionais não são turbo");
    assert!(
        public
            .notes
            .iter()
            .all(|note| note.title != "Partidas profissionais não são turbo"),
        "a vitória pública de Turbo é de Turbo; não há o que avisar"
    );
}

#[test]
fn professional_build_outside_turbo_has_no_turbo_warning() {
    let recommendations = listed(GameMode::AllPick, vec![pro_win(1, zeus_build(), None)]);
    let pro = detail(&recommendations, &PRO, None).unwrap();
    assert!(pro.notes.is_empty());
}

fn learned(levels: &[(&str, u32)], talents: u32) -> GameSnapshot {
    GameSnapshot {
        abilities: Some(
            levels
                .iter()
                .map(|(name, level)| AbilityState {
                    name: (*name).into(),
                    level: *level,
                    ultimate: false,
                })
                .collect(),
        ),
        talents_learned: Some(talents),
        ..GameSnapshot::menu()
    }
}

#[test]
fn ability_grid_follows_the_real_order_with_the_points_spent() {
    let recommendations = listed(
        GameMode::AllPick,
        vec![pro_win(
            1,
            zeus_build(),
            Some(vec![5111, 5110, 5111, 6000, 9]),
        )],
    );
    let game = learned(&[("zuus_lightning_bolt", 1), ("zuus_arc_lightning", 1)], 0);

    let grid = detail(&recommendations, &PRO, Some(&game))
        .unwrap()
        .abilities
        .unwrap();

    assert_eq!(
        grid.images,
        [
            Some("zuus_lightning_bolt".to_owned()),
            Some("zuus_arc_lightning".to_owned()),
            None,
            None
        ],
        "talento e habilidade desconhecida ficam sem imagem"
    );
    assert_eq!(
        grid.rows,
        [
            "Lightning Bolt",
            "Arc Lightning",
            "Talentos",
            "habilidade 9"
        ]
    );
    assert_eq!(
        grid.points,
        [
            (0, StepState::Done),
            (1, StepState::Done),
            (0, StepState::Next),
            (2, StepState::Pending),
            (3, StepState::Pending),
        ]
    );
    assert!(
        grid.summary
            .starts_with("ponto 1: Lightning Bolt; ponto 2: Arc Lightning")
    );
}

#[test]
fn ability_grid_without_game_data_is_all_pending() {
    let recommendations = listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), Some(vec![5111, 5110]))],
    );

    let grid = detail(&recommendations, &PRO, None)
        .unwrap()
        .abilities
        .unwrap();

    assert!(
        grid.points
            .iter()
            .all(|(_, state)| *state == StepState::Pending)
    );
}

#[test]
fn item_chip_reads_name_time_and_state() {
    let chip = ItemChip {
        item: "item_blink".into(),
        label: "Blink Dagger".into(),
        time: Some(754),
        state: Some(StepState::Next),
    };
    assert_eq!(
        chip.accessible_label(),
        "Blink Dagger, comprado em 12:34, próximo"
    );
    assert_eq!(chip.marker(), "→");
}

#[test]
fn sidebar_rows_put_a_title_before_each_group_and_map_rows_to_builds() {
    let list = sidebar_of(&ready(listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), None)],
    )));

    let items = sidebar_items(&list);

    assert!(matches!(&items[0], SidebarItem::Group(title) if title == "Vitórias profissionais"));
    assert!(matches!(&items[2], SidebarItem::Group(title) if title == "Vitórias públicas"));
    assert_eq!(entry_at(&items, 1), Some(PRO));
    assert_eq!(entry_at(&items, 3), Some(PUBLIC));
    assert_eq!(entry_at(&items, 0), None, "título não é build");
    assert_eq!(entry_at(&items, 9), None, "fora da lista");
    assert_eq!(entry_at(&items, -1), None);
    assert_eq!(index_of(&items, &PUBLIC), Some(3));
}

#[test]
fn empty_sidebar_has_only_the_explanation() {
    let items = sidebar_items(&sidebar_of(&RecommendationStatus::Idle));
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], SidebarItem::Empty(_)));
    assert_eq!(entry_at(&items, 0), None);
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
fn purchase_before_the_start_has_no_clock() {
    assert_eq!(purchase_time(-89), "antes do início");
    assert_eq!(purchase_time(1300), "21:40");
}

#[test]
fn ability_names_fall_back_to_internal_name_and_id() {
    let abilities = AbilityIdCatalog::new(HashMap::from([
        (5110, "zuus_arc_lightning".to_owned()),
        (7000, "zuus_new_ability".to_owned()),
        (6000, "special_bonus_spell_amplify_8".to_owned()),
    ]));
    assert_eq!(ability_name(5110, &heroes(), &abilities), "Arc Lightning");
    assert_eq!(
        ability_name(7000, &heroes(), &abilities),
        "zuus_new_ability"
    );
    assert_eq!(ability_name(6000, &heroes(), &abilities), "Talento");
    assert_eq!(ability_name(1, &heroes(), &abilities), "habilidade 1");
}

#[test]
fn loading_header_names_the_hero_and_mode() {
    let loading = RecommendationStatus::Loading {
        key: crate::application::use_cases::recommendation_request::RecommendationKey {
            hero_id: 22,
            mode: GameMode::Turbo,
            patch_id: 60,
        },
    };
    assert_eq!(
        header(&loading),
        vec![RowContent::new(
            Level::Info,
            "Carregando recomendações de Zeus",
            "Turbo"
        )]
    );
}

fn three_wins() -> Sidebar {
    sidebar_of(&ready(listed(
        GameMode::AllPick,
        vec![
            pro_win(1, zeus_build(), None),
            pro_win(3, zeus_build(), None),
        ],
    )))
}

fn builds_of(sidebar: &Sidebar) -> Vec<BuildId> {
    sidebar.entries().map(|entry| entry.id).collect()
}

fn key() -> RecommendationKey {
    RecommendationKey {
        hero_id: 22,
        mode: GameMode::AllPick,
        patch_id: 60,
    }
}

fn pro_id(match_id: u64) -> BuildId {
    BuildId {
        source: BuildSource::Pro,
        match_id,
    }
}

#[test]
fn selection_is_kept_while_the_build_stays_in_the_list() {
    let items = sidebar_items(&three_wins());
    let id = builds_of(&three_wins())[1];
    assert_eq!(select_build(Some(&(key(), id)), &key(), &items), Some(id));
}

#[test]
fn selection_falls_back_to_the_first_build() {
    let items = sidebar_items(&three_wins());
    let first = builds_of(&three_wins())[0];
    assert_eq!(select_build(None, &key(), &items), Some(first));
    assert_eq!(
        select_build(Some(&(key(), pro_id(999))), &key(), &items),
        Some(first),
        "a build que não existe mais"
    );
    let other_key = RecommendationKey {
        hero_id: 5,
        ..key()
    };
    assert_eq!(
        select_build(Some(&(other_key, pro_id(1))), &key(), &items),
        Some(first),
        "outra consulta"
    );
}

#[test]
fn no_builds_means_no_selection() {
    let items = sidebar_items(&sidebar_of(&RecommendationStatus::Idle));
    assert_eq!(select_build(None, &key(), &items), None);
}

#[test]
fn sidebar_entries_carry_the_result_of_the_player() {
    let kda = Kda {
        kills: 3,
        deaths: 8,
        assists: 16,
    };
    let mut pro = pro_win(1, zeus_build(), None);
    pro.hero_match.kda = Some(kda);
    let mut recommendations = listed(GameMode::AllPick, vec![pro]);
    recommendations.public_wins[0].kda = Some(Kda {
        kills: 10,
        deaths: 0,
        assists: 2,
    });

    let list = sidebar_of(&ready(recommendations));

    assert_eq!(list.pro[0].kda, Some(kda));
    assert_eq!(kda_text(kda), "3/8/16");
    assert_eq!(
        kda_label(kda),
        "abates 3, mortes 8, assistências 16",
        "o leitor de tela lê por extenso"
    );
    assert_eq!(list.public[0].kda.map(kda_text).as_deref(), Some("10/0/2"));
}

#[test]
fn sidebar_entry_without_result_has_none() {
    let list = sidebar_of(&ready(listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), None)],
    )));
    assert_eq!(list.pro[0].kda, None);
}

#[test]
fn public_entry_without_rank_says_so() {
    let mut recommendations = listed(GameMode::AllPick, vec![]);
    recommendations.public_wins[0].avg_rank_tier = None;

    let list = sidebar_of(&ready(recommendations));

    assert_eq!(list.public[0].title, "Rank médio não informado");
}

#[test]
fn only_professional_builds_with_totals_have_a_comparison() {
    let mut pro = pro_win(1, zeus_build(), None);
    pro.stats = crate::domain::match_data::FinalStats {
        last_hits: Some(184),
        gpm: Some(469),
        ..Default::default()
    };
    pro.hero_match.duration_secs = Some(2042);
    let recommendations = listed(GameMode::AllPick, vec![pro, pro_win(3, zeus_build(), None)]);

    let with_totals = detail(&recommendations, &PRO, None).unwrap();
    let without_totals = detail(
        &recommendations,
        &BuildId {
            source: BuildSource::Pro,
            match_id: 3,
        },
        None,
    )
    .unwrap();
    let public = detail(&recommendations, &PUBLIC, None).unwrap();

    let comparison = with_totals
        .comparison
        .expect("a profissional com totais compara");
    assert_eq!(comparison.rows.len(), 4);
    assert_eq!(
        without_totals.comparison, None,
        "sem totais não há o que comparar"
    );
    assert_eq!(
        public.comparison, None,
        "as públicas guardadas não têm os totais"
    );
}
