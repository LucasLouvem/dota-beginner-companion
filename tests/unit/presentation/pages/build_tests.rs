//! Testes unitários de `src/presentation/pages/build.rs` (conteúdo da página
//! da build, sem widgets).

use super::*;
use crate::application::ports::ProviderError;
use crate::application::state::ModeOrigin;
use crate::application::use_cases::pro_builds::ProWin;
use crate::domain::builds::PlayerBuild;
use crate::domain::game::{AbilityState, CarriedItem};
use crate::domain::hero_catalog::{AbilityInfo, AttackType, HeroInfo, PrimaryAttribute};
use crate::domain::item_catalog::ItemInfo;
use crate::test_support::{pro_builds, pro_win, public_win, recommendations, zeus_build};
use std::collections::HashMap;
use std::sync::Arc;

fn heroes() -> HeroCatalog {
    HeroCatalog::new(vec![HeroInfo {
        id: 22,
        name: "npc_dota_hero_zuus".into(),
        display_name: "Zeus".into(),
        primary_attribute: PrimaryAttribute::Intelligence,
        attack_type: AttackType::Ranged,
        roles: vec![],
        abilities: vec![
            AbilityInfo {
                name: "zuus_arc_lightning".into(),
                display_name: "Arc Lightning".into(),
            },
            AbilityInfo {
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

fn confirmed(mode: GameMode) -> ModeState {
    ModeState::Confirmed {
        mode,
        origin: ModeOrigin::Chosen,
    }
}

fn titles(rows: &[RowContent]) -> Vec<&str> {
    rows.iter().map(|row| row.title.as_str()).collect()
}

fn header(status: &RecommendationStatus, mode: &ModeState) -> Vec<RowContent> {
    header_rows(status, mode, &heroes(), &Redactor::default())
}

#[test]
fn header_shows_hero_mode_patch_counts_and_few_wins() {
    let rows = header(
        &ready(listed(
            GameMode::AllPick,
            vec![pro_win(1, zeus_build(), None)],
        )),
        &confirmed(GameMode::AllPick),
    );

    assert_eq!(
        titles(&rows),
        [
            "Zeus · Escolha Livre · patch 7.41",
            "1 vitórias profissionais · 1 vitórias públicas",
            "Poucas vitórias encontradas",
        ]
    );
    assert!(rows[1].subtitle.contains("patch 7.41"), "{:?}", rows[1]);
    assert!(
        rows[1].subtitle.contains("8 partidas de liga"),
        "{:?}",
        rows[1]
    );
}

#[test]
fn header_warns_about_mode_not_informed_turbo_and_bots() {
    let turbo = header(
        &ready(listed(GameMode::Turbo, vec![])),
        &ModeState::Awaiting {
            preselected: GameMode::Turbo,
        },
    );
    let bots = header(
        &ready(listed(GameMode::Bots, vec![])),
        &confirmed(GameMode::Bots),
    );

    assert!(titles(&turbo).contains(&"Modo ainda não informado; mostrando dados de Turbo"));
    assert!(titles(&turbo).contains(&"Partidas profissionais não são turbo"));
    assert!(titles(&bots).contains(
        &"Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores."
    ));
    assert!(
        !titles(&bots)
            .iter()
            .any(|title| title.starts_with("Modo ainda"))
    );
}

#[test]
fn header_marks_old_data_and_shows_provider_errors() {
    let mut stale = listed(GameMode::AllPick, vec![]);
    stale.pro.candidates_origin = Some(DataOrigin::Stale);
    stale.pro.error = Some(ProviderError::RateLimited {
        path: "/heroes/22/matches".into(),
    });

    let rows = header(&ready(stale), &confirmed(GameMode::AllPick));

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
        titles(&header(&RecommendationStatus::Idle, &ModeState::NotAsked)),
        ["Nenhum herói para recomendar"]
    );
    assert_eq!(
        titles(&header(
            &RecommendationStatus::Unavailable,
            &ModeState::NotAsked
        )),
        ["Recomendações indisponíveis"]
    );
}

#[test]
fn sidebar_lists_professional_then_public_wins_with_final_items() {
    let list = sidebar(&ready(listed(
        GameMode::AllPick,
        vec![pro_win(1, zeus_build(), None)],
    )));

    assert_eq!(list.pro[0].id, PRO);
    assert_eq!(list.pro[0].title, "Liga Teste");
    assert_eq!(list.pro[0].subtitle, "40:00");
    assert_eq!(list.pro[0].items, ["item_blink", "item_black_king_bar"]);
    assert_eq!(list.public[0].id, PUBLIC);
    assert_eq!(list.public[0].title, "Partida pública · Lenda 1");
    assert_eq!(list.empty, None);
    assert_eq!(list.entries().count(), 2);
}

#[test]
fn sidebar_without_wins_or_hero_explains_why() {
    let no_wins = sidebar(&ready(recommendations(
        GameMode::AllPick,
        pro_builds(vec![], 12, None),
        vec![],
    )));
    let no_hero = sidebar(&RecommendationStatus::Idle);

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
    assert_eq!(
        header(&status, &confirmed(GameMode::AllPick)),
        header(&status, &confirmed(GameMode::AllPick))
    );
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
    assert_eq!(build.starting[0].time.as_deref(), Some("antes do início"));
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
    assert_eq!(build.neutral[1].label, "Greedy");
    assert_eq!(build.backpack[0].label, "Smoke of Deceit");
    assert_eq!(build.windows[0].0, "Antes do início");
    assert_eq!(build.windows[1].0, "De 10 a 25 minutos");
    assert_eq!(build.windows[1].1[0].time.as_deref(), Some("12:34"));
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
    assert!(build.windows.is_empty());
    assert_eq!(build.order.len(), 3);
    assert_eq!(build.abilities, None);
    assert_eq!(
        build.abilities_note.unwrap().title,
        "Sem ordem de habilidades"
    );
}

#[test]
fn professional_build_in_turbo_is_never_shown_as_turbo() {
    let recommendations = listed(GameMode::Turbo, vec![pro_win(1, zeus_build(), None)]);

    let pro = detail(&recommendations, &PRO, None).unwrap();
    let public = detail(&recommendations, &PUBLIC, None).unwrap();

    assert_eq!(pro.notes[0].title, "Partidas profissionais não são turbo");
    assert!(
        !public
            .notes
            .iter()
            .any(|note| note.title == "Partidas profissionais não são turbo")
    );
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
    assert_eq!(
        grid.max_order,
        "Arc Lightning > Lightning Bolt > habilidade 9"
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
        time: Some("12:34".into()),
        state: Some(StepState::Next),
    };
    assert_eq!(
        chip.accessible_label(),
        "Blink Dagger, comprado em 12:34, próximo"
    );
    assert_eq!(chip.marker(), "→");
}

#[test]
fn progress_row_waits_for_hero_data() {
    assert_eq!(
        progress_row(None).subtitle,
        "O progresso aparece quando o Dota envia os dados do herói."
    );
    let game = GameSnapshot {
        hero_level: Some(12),
        ..GameSnapshot::menu()
    };
    assert!(progress_row(Some(&game)).subtitle.contains("12"));
}

#[test]
fn every_window_has_a_label() {
    let labels: Vec<String> = [
        PurchaseWindow::BeforeStart,
        PurchaseWindow::Early,
        PurchaseWindow::Mid,
        PurchaseWindow::Late,
    ]
    .into_iter()
    .map(window_label)
    .collect();
    assert_eq!(
        labels,
        [
            "Antes do início",
            "Até 10 minutos",
            "De 10 a 25 minutos",
            "Depois de 25 minutos"
        ]
    );
}
