//! Testes unitários de `src/presentation/pages/build_compare.rs`.

use super::*;
use crate::domain::game::PlayerStats;

fn build() -> FinalStats {
    FinalStats {
        last_hits: Some(300),
        denies: Some(30),
        gpm: Some(500),
        xpm: Some(600),
        net_worth: Some(20_000),
        observers_placed: Some(4),
        sentries_placed: Some(2),
    }
}

/// Você aos 10 minutos de partida.
fn game(stats: PlayerStats, clock: i32) -> GameSnapshot {
    GameSnapshot {
        clock_time: Some(clock),
        stats,
        ..GameSnapshot::menu()
    }
}

#[test]
fn compare_tells_ahead_behind_and_even_with_a_tolerance() {
    assert_eq!(
        compare(Some(120.0), Some(100.0)),
        (Verdict::Ahead, "+20%".into())
    );
    assert_eq!(
        compare(Some(80.0), Some(100.0)),
        (Verdict::Behind, "-20%".into())
    );
    assert_eq!(compare(Some(102.0), Some(100.0)).0, Verdict::Even);
    assert_eq!(compare(Some(97.5), Some(100.0)).0, Verdict::Even);
}

#[test]
fn compare_needs_both_numbers_and_survives_a_zero_build() {
    assert_eq!(compare(None, Some(1.0)), (Verdict::Unknown, String::new()));
    assert_eq!(compare(Some(1.0), None), (Verdict::Unknown, String::new()));
    assert_eq!(
        compare(Some(5.0), Some(0.0)),
        (Verdict::Ahead, String::new())
    );
    assert_eq!(
        compare(Some(0.0), Some(0.0)),
        (Verdict::Even, String::new())
    );
}

#[test]
fn rates_are_per_minute_of_each_game() {
    // Build: 300 last hits em 30 min = 10/min. Você: 60 em 10 min = 6/min.
    let you = game(
        PlayerStats {
            last_hits: Some(60),
            denies: Some(10),
            gpm: Some(450),
            xpm: Some(660),
            ..PlayerStats::default()
        },
        600,
    );

    let comparison = comparison(&build(), Some(1800), Some(&you)).unwrap();

    let by_label = |label: &str| {
        comparison
            .rows
            .iter()
            .find(|row| row.label == label)
            .unwrap()
    };
    let last_hits = by_label("Last hits/min");
    assert_eq!(
        (last_hits.you.as_str(), last_hits.build.as_str()),
        ("6,0", "10,0")
    );
    assert_eq!(last_hits.verdict, Verdict::Behind);
    assert_eq!(last_hits.diff, "-40%");
    let denies = by_label("Denies/min");
    assert_eq!((denies.you.as_str(), denies.build.as_str()), ("1,0", "1,0"));
    assert_eq!(denies.verdict, Verdict::Even);
    let gpm = by_label("Ouro/min (GPM)");
    assert_eq!((gpm.you.as_str(), gpm.build.as_str()), ("450", "500"));
    assert_eq!(gpm.verdict, Verdict::Behind);
    assert_eq!(by_label("XP/min (XPM)").verdict, Verdict::Ahead);
    assert!(comparison.note.is_empty());
}

#[test]
fn before_two_minutes_or_without_a_game_your_numbers_are_missing() {
    let early = game(
        PlayerStats {
            last_hits: Some(3),
            ..PlayerStats::default()
        },
        60,
    );
    for you in [None, Some(&early)] {
        let comparison = comparison(&build(), Some(1800), you).unwrap();
        assert!(comparison.rows.iter().all(|row| row.you == "–"));
        assert!(
            comparison
                .rows
                .iter()
                .all(|row| row.verdict == Verdict::Unknown)
        );
        assert!(comparison.note.starts_with("As suas médias aparecem"));
    }
}

#[test]
fn a_build_without_any_total_has_no_comparison() {
    assert_eq!(comparison(&FinalStats::default(), Some(1800), None), None);
}

#[test]
fn without_duration_the_build_has_only_the_averages_it_already_has() {
    let you = game(PlayerStats::default(), 600);
    let comparison = comparison(&build(), None, Some(&you)).unwrap();
    let last_hits = &comparison.rows[0];
    assert_eq!(last_hits.build, "–", "sem duração não há média por minuto");
    let gpm = &comparison.rows[2];
    assert_eq!(gpm.build, "500");
}

#[test]
fn verdicts_read_with_a_symbol() {
    assert_eq!(Verdict::Ahead.label(), "▲ à frente");
    assert_eq!(Verdict::Behind.label(), "▼ atrás");
    assert_eq!(Verdict::Even.label(), "= igual");
    assert_eq!(Verdict::Unknown.label(), "");
}
