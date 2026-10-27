//! Comparação entre as suas médias na partida e as médias finais do jogador
//! da build escolhida, sem widgets. Só números por minuto e totais finais: o
//! GSI informa os seus e o OpenDota, os dele. Não compara abates, mortes e
//! assistências.

use crate::domain::game::GameSnapshot;
use crate::domain::match_data::FinalStats;
use crate::presentation::i18n::translate;

/// Tempo de partida (em segundos) a partir do qual as suas médias valem;
/// antes disso, uma média por minuto não diz nada.
pub const MIN_CLOCK_SECS: i32 = 120;
/// Diferença, em porcentagem, abaixo da qual os dois valem o mesmo.
const EVEN_TOLERANCE_PERCENT: f64 = 3.0;

/// Como você está em relação à build numa métrica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ahead,
    Behind,
    Even,
    /// Falta o seu número ou o da build.
    Unknown,
}

impl Verdict {
    /// Símbolo e texto, para o resultado não depender só da cor.
    pub fn label(self) -> String {
        match self {
            Verdict::Ahead => translate("▲ à frente"),
            Verdict::Behind => translate("▼ atrás"),
            Verdict::Even => translate("= igual"),
            Verdict::Unknown => String::new(),
        }
    }
}

/// Uma métrica: o seu valor, o da build e a diferença.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRow {
    pub label: String,
    pub you: String,
    pub build: String,
    pub verdict: Verdict,
    /// Diferença em porcentagem ("+12%"), vazia sem os dois números.
    pub diff: String,
}

/// A comparação mostrada na página.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub rows: Vec<CompareRow>,
    /// Aviso quando as suas médias ainda não valem.
    pub note: String,
}

/// Compara dois valores; a build igual a zero não permite porcentagem.
pub fn compare(you: Option<f64>, build: Option<f64>) -> (Verdict, String) {
    let (Some(you), Some(build)) = (you, build) else {
        return (Verdict::Unknown, String::new());
    };
    if build <= 0.0 {
        let verdict = if you > 0.0 {
            Verdict::Ahead
        } else {
            Verdict::Even
        };
        return (verdict, String::new());
    }
    let percent = (you - build) / build * 100.0;
    let verdict = if percent.abs() < EVEN_TOLERANCE_PERCENT {
        Verdict::Even
    } else if percent > 0.0 {
        Verdict::Ahead
    } else {
        Verdict::Behind
    };
    (verdict, format!("{percent:+.0}%"))
}

/// Média por minuto; `None` sem tempo de partida.
fn per_minute(total: Option<u32>, seconds: Option<i32>) -> Option<f64> {
    let seconds = seconds.filter(|seconds| *seconds > 0)?;
    Some(f64::from(total?) * 60.0 / f64::from(seconds))
}

fn decimal(value: Option<f64>) -> String {
    value.map_or_else(
        || "–".to_owned(),
        |value| format!("{value:.1}").replace('.', ","),
    )
}

fn whole(value: Option<f64>) -> String {
    value.map_or_else(|| "–".to_owned(), |value| format!("{value:.0}"))
}

fn row(
    label: &str,
    you: Option<f64>,
    build: Option<f64>,
    show: impl Fn(Option<f64>) -> String,
) -> CompareRow {
    let (verdict, diff) = compare(you, build);
    CompareRow {
        label: translate(label),
        you: show(you),
        build: show(build),
        verdict,
        diff,
    }
}

/// Comparação das suas médias (`game`) com os totais finais da build, cuja
/// partida durou `duration_secs`. `None` quando a build não tem nenhum total.
pub fn comparison(
    build: &FinalStats,
    duration_secs: Option<u32>,
    game: Option<&GameSnapshot>,
) -> Option<Comparison> {
    if build.is_empty() {
        return None;
    }
    let build_seconds = duration_secs.and_then(|secs| i32::try_from(secs).ok());
    let clock = game
        .and_then(|game| game.clock_time)
        .filter(|clock| *clock >= MIN_CLOCK_SECS);
    let yours = game.filter(|_| clock.is_some()).map(|game| &game.stats);
    let you_per_minute = |total: Option<u32>| per_minute(total, clock);
    let rows = vec![
        row(
            "Last hits/min",
            yours.and_then(|stats| you_per_minute(stats.last_hits)),
            per_minute(build.last_hits, build_seconds),
            decimal,
        ),
        row(
            "Denies/min",
            yours.and_then(|stats| you_per_minute(stats.denies)),
            per_minute(build.denies, build_seconds),
            decimal,
        ),
        row(
            "Ouro/min (GPM)",
            yours.and_then(|stats| stats.gpm).map(f64::from),
            build.gpm.map(f64::from),
            whole,
        ),
        row(
            "XP/min (XPM)",
            yours.and_then(|stats| stats.xpm).map(f64::from),
            build.xpm.map(f64::from),
            whole,
        ),
    ];
    let note = if clock.is_some() {
        String::new()
    } else {
        translate(
            "As suas médias aparecem a partir dos 2 minutos de partida; a build mostra a média da partida inteira.",
        )
    };
    Some(Comparison { rows, note })
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/build_compare_tests.rs"]
mod tests;
