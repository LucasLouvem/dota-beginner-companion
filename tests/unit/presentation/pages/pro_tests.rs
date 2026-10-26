//! Testes unitários de `src/presentation/pages/pro.rs`.

use super::*;
use crate::application::use_cases::fallback::DataOrigin;
use crate::domain::game::GameMode;
use crate::test_support::{pro_builds, recommendations};
use std::sync::Arc;

fn recent(match_id: u64, league_id: Option<u32>, start_time: i64, won: Option<bool>) -> HeroMatch {
    HeroMatch {
        match_id,
        start_time: Some(start_time),
        league_id,
        league_name: Some(format!("Liga {match_id}")),
        hero_won: won,
        duration_secs: Some(2042),
    }
}

fn ready(matches: Vec<HeroMatch>, origin: Option<DataOrigin>) -> RecommendationStatus {
    let mut pro = pro_builds(vec![], 0, origin);
    pro.recent_matches = matches;
    RecommendationStatus::Ready(Arc::new(recommendations(GameMode::AllPick, pro, vec![])))
}

fn rows(status: &RecommendationStatus) -> Vec<RowContent> {
    let mut sections = pro_content(status, &HeroCatalog::default(), &Redactor::default());
    assert_eq!(sections.len(), 2);
    sections.remove(1)
}

#[test]
fn shows_recent_league_matches_newest_first_with_the_hero_result() {
    let status = ready(
        vec![
            recent(1, Some(10), 1_792_000_000, Some(false)),
            recent(2, Some(10), 1_792_590_894, Some(true)),
            recent(3, None, 1_792_900_000, Some(true)),
        ],
        Some(DataOrigin::Live),
    );

    let match_rows = rows(&status);

    let titles: Vec<&str> = match_rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles[..2], ["Liga 2: vitória", "Liga 1: derrota"]);
    assert!(
        match_rows[0].subtitle.ends_with("duração 34:02"),
        "{:?}",
        match_rows[0]
    );
}

#[test]
fn at_most_ten_matches_are_listed() {
    let matches = (1..=15)
        .map(|id| recent(id, Some(10), 1_792_000_000 + id as i64, Some(true)))
        .collect();

    let match_rows = rows(&ready(matches, Some(DataOrigin::Live)));

    assert_eq!(
        match_rows.len(),
        RECENT_MATCHES_SHOWN + 1,
        "e a linha da fonte"
    );
}

#[test]
fn without_league_matches_says_so_and_marks_missing_data() {
    let match_rows = rows(&ready(vec![], None));

    assert_eq!(
        match_rows[0].title,
        "Nenhuma partida profissional recente do herói"
    );
    assert_eq!(match_rows[1].subtitle, "Sem dados");
}

#[test]
fn match_without_result_or_time_is_still_listed() {
    let row = match_row(&HeroMatch {
        match_id: 1,
        start_time: None,
        league_id: Some(10),
        league_name: None,
        hero_won: None,
        duration_secs: None,
    });
    assert_eq!(row.title, "Liga sem nome: resultado não informado");
    assert_eq!(row.subtitle, "");
}
