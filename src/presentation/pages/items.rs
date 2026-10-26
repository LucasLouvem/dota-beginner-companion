//! Conteúdo da aba "Itens": a build real das vitórias profissionais e
//! públicas mais recentes do herói no modo da partida, e a popularidade de
//! itens por fase. Cada bloco diz a fonte, o patch e se os dados são antigos.
//! Também as linhas de estado compartilhadas pelas abas de recomendação.

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::ports::PublicWin;
use crate::application::state::RecommendationStatus;
use crate::application::use_cases::fallback::DataOrigin;
use crate::application::use_cases::pro_builds::{PRO_WINS_SHOWN, ProWin};
use crate::application::use_cases::recommendations::{PUBLIC_WINS_SHOWN, Recommendations};
use crate::domain::builds::PlayerBuild;
use crate::domain::game::GameMode;
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::PopularityPhase;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{Level, RowContent, format_date, format_duration, format_time};

use super::hero::mode_label;

/// Itens mostrados por fase na popularidade.
const POPULAR_ITEMS_SHOWN: usize = 5;

/// Conteúdo das seções da aba, na ordem exibida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemsContent {
    pub header: Vec<RowContent>,
    pub notes: Vec<RowContent>,
    pub pro: Vec<RowContent>,
    pub public: Vec<RowContent>,
    pub popularity: Vec<RowContent>,
}

impl ItemsContent {
    /// Seções na ordem da aba.
    pub fn into_sections(self) -> Vec<Vec<RowContent>> {
        vec![
            self.header,
            self.notes,
            self.pro,
            self.public,
            self.popularity,
        ]
    }
}

/// Nome de exibição do herói, ou o ID quando o catálogo não o conhece.
pub fn hero_name(heroes: &HeroCatalog, hero_id: u32) -> String {
    heroes.by_id(hero_id).map_or_else(
        || translate_with("herói {}", &[&hero_id.to_string()]),
        |hero| hero.display_name.clone(),
    )
}

/// Linhas do cabeçalho das abas de recomendação enquanto não há dados
/// prontos; `None` quando as recomendações estão prontas.
pub fn status_rows(
    status: &RecommendationStatus,
    heroes: &HeroCatalog,
    redactor: &Redactor,
) -> Option<Vec<RowContent>> {
    match status {
        RecommendationStatus::Unavailable => Some(vec![RowContent::new(
            Level::Warning,
            &translate("Recomendações indisponíveis"),
            &translate(
                "O banco local ou o cliente HTTP não puderam ser abertos; os detalhes estão no log.",
            ),
        )]),
        RecommendationStatus::Idle => Some(vec![RowContent::new(
            Level::Info,
            &translate("Nenhum herói para recomendar"),
            &translate("As recomendações aparecem quando o Dota informa o herói escolhido."),
        )]),
        RecommendationStatus::Loading { key } => Some(vec![RowContent::new(
            Level::Info,
            &translate_with(
                "Carregando recomendações de {}",
                &[&hero_name(heroes, key.hero_id)],
            ),
            &mode_label(key.mode),
        )]),
        RecommendationStatus::Interrupted { interrupted, .. } => {
            Some(vec![RowContent::diagnostic(
                &interrupted.to_diagnostic(redactor),
            )])
        }
        RecommendationStatus::Ready(_) => None,
    }
}

/// Cabeçalho das abas de recomendação com os dados prontos.
pub fn ready_header(recommendations: &Recommendations, heroes: &HeroCatalog) -> RowContent {
    RowContent::new(
        Level::Info,
        &translate_with(
            "{} · {} · patch {}",
            &[
                &hero_name(heroes, recommendations.key.hero_id),
                &mode_label(recommendations.key.mode),
                &recommendations.patch.name,
            ],
        ),
        &translate_with(
            "Consultado às {}",
            &[&format_time(recommendations.loaded_at)],
        ),
    )
}

/// Aviso das abas que usam partidas profissionais quando o modo é Turbo.
pub fn turbo_warning(subtitle: &str) -> RowContent {
    RowContent::new(
        Level::Warning,
        &translate("Partidas profissionais não são turbo"),
        subtitle,
    )
}

/// Aviso do modo Contra bots, que usa os dados de Escolha Livre.
pub fn bots_note() -> RowContent {
    RowContent::new(
        Level::Info,
        &translate("Modo Contra bots: as builds vêm de partidas de Escolha Livre entre jogadores."),
        &translate("Não há dados de partidas contra bots nas recomendações."),
    )
}

/// Conteúdo da aba Itens para o estado das recomendações.
pub fn items_content(
    status: &RecommendationStatus,
    heroes: &HeroCatalog,
    items: &ItemCatalog,
    redactor: &Redactor,
) -> ItemsContent {
    match status {
        RecommendationStatus::Ready(recommendations) => {
            ready_content(recommendations.as_ref(), heroes, items, redactor)
        }
        _ => ItemsContent {
            header: status_rows(status, heroes, redactor).unwrap_or_default(),
            ..ItemsContent::default()
        },
    }
}

fn ready_content(
    recommendations: &Recommendations,
    heroes: &HeroCatalog,
    items: &ItemCatalog,
    redactor: &Redactor,
) -> ItemsContent {
    let mode = recommendations.key.mode;
    let patch = &recommendations.patch.name;
    let header = vec![ready_header(recommendations, heroes)];
    let mut notes = vec![RowContent::new(
        Level::Info,
        &translate("As vitórias públicas são de todas as faixas de rank e de qualquer posição."),
        &translate(
            "A posição (1 a 5) não vem do Dota, então as partidas não são separadas por ela.",
        ),
    )];
    if mode == GameMode::Bots {
        notes.push(bots_note());
    }

    let mut pro = Vec::new();
    if mode == GameMode::Turbo {
        pro.push(turbo_warning(&translate(
            "Os minutos de compra destas partidas não valem para o ritmo do turbo.",
        )));
    }
    let pro_builds = &recommendations.pro;
    pro.extend(pro_builds.wins.iter().flat_map(|win| {
        let (heading, detail) = pro_win_heading(win);
        win_rows(&heading, &detail, &win.build, items)
    }));
    if let Some(found) = found_row(
        pro_builds.wins.len(),
        PRO_WINS_SHOWN,
        &examined_note(pro_builds.examined),
    ) {
        pro.push(found);
    }
    pro.push(source_row(
        &translate_with(
            "Fonte: OpenDota, partidas profissionais de liga · patch {}",
            &[patch],
        ),
        pro_builds.candidates_origin,
    ));
    if let Some(error) = &pro_builds.error {
        pro.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }

    let mut public: Vec<RowContent> = recommendations
        .public_wins
        .iter()
        .flat_map(|win| public_win_rows(win, items))
        .collect();
    if let Some(found) = found_row(
        recommendations.public_wins.len(),
        PUBLIC_WINS_SHOWN,
        &translate(
            "A coleta em segundo plano ainda não gravou mais vitórias do herói neste modo e patch.",
        ),
    ) {
        public.push(found);
    }
    public.push(RowContent::new(
        Level::Info,
        &translate_with(
            "Fonte: partidas públicas coletadas do OpenDota · patch {}",
            &[patch],
        ),
        &translate("A coleta continua em segundo plano enquanto o aplicativo está aberto."),
    ));
    if let Some(error) = &recommendations.public_error {
        public.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }

    ItemsContent {
        header,
        notes,
        pro,
        public,
        popularity: popularity_rows(recommendations, items, redactor),
    }
}

/// Linhas de uma vitória pública: a partida, com a data e o rank médio, e a
/// build real.
fn public_win_rows(win: &PublicWin, items: &ItemCatalog) -> Vec<RowContent> {
    let mut details = Vec::new();
    if let Some(start_time) = win.start_time {
        details.push(format_date(start_time));
    }
    if let Some(rank) = win.avg_rank_tier.and_then(rank_label) {
        details.push(translate_with("rank médio {}", &[&rank]));
    }
    win_rows(
        &translate("Vitória em partida pública"),
        &details.join(" · "),
        &win.build,
        items,
    )
}

/// Linhas de uma vitória: o cabeçalho da partida, o inventário final (com a
/// mochila e o neutro) e a ordem de compra completa, como o jogador fez.
pub fn win_rows(
    heading: &str,
    detail: &str,
    build: &PlayerBuild,
    items: &ItemCatalog,
) -> Vec<RowContent> {
    let names = |list: &[String]| -> String {
        list.iter()
            .map(|item| items.display_name(item))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let inventory = if build.inventory.is_empty() {
        translate("Inventário final vazio")
    } else {
        translate_with("Inventário final: {}", &[&names(&build.inventory)])
    };
    let backpack = if build.backpack.is_empty() {
        translate("Mochila vazia")
    } else {
        translate_with("Mochila: {}", &[&names(&build.backpack)])
    };
    let neutral = match (&build.neutral, &build.neutral_enhancement) {
        (Some(neutral), Some(enhancement)) => translate_with(
            "Neutro: {} + {}",
            &[items.display_name(neutral), items.display_name(enhancement)],
        ),
        (Some(neutral), None) => translate_with("Neutro: {}", &[items.display_name(neutral)]),
        (None, Some(enhancement)) => translate_with(
            "Aprimoramento neutro: {}",
            &[items.display_name(enhancement)],
        ),
        (None, None) => translate("Sem item neutro"),
    };
    let purchases = match &build.purchases {
        Some(purchases) if !purchases.is_empty() => RowContent::new(
            Level::Info,
            &translate("Ordem de compra"),
            &purchases
                .iter()
                .map(|purchase| {
                    format!(
                        "{} ({})",
                        items.display_name(&purchase.item),
                        purchase_time(purchase.time)
                    )
                })
                .collect::<Vec<_>>()
                .join(" → "),
        ),
        _ => RowContent::new(
            Level::Info,
            &translate("Sem ordem de compra"),
            &translate(
                "A partida não foi analisada pelo OpenDota; só o inventário final está disponível.",
            ),
        ),
    };
    vec![
        RowContent::new(Level::Info, heading, detail),
        RowContent::new(Level::Info, &inventory, &format!("{backpack} · {neutral}")),
        purchases,
    ]
}

/// Linha com quantas vitórias recentes foram encontradas, quando são menos
/// que as `shown` mostradas; `None` quando a lista está completa.
pub fn found_row(found: usize, shown: usize, explanation: &str) -> Option<RowContent> {
    let title = match found {
        _ if found >= shown => return None,
        0 => translate("Nenhuma vitória recente encontrada"),
        1 => translate("1 vitória recente encontrada"),
        _ => translate_with("{} vitórias recentes encontradas", &[&found.to_string()]),
    };
    Some(RowContent::new(Level::Info, &title, explanation))
}

/// Medalha do rank médio (`avg_rank_tier` do OpenDota: a dezena é a medalha,
/// de Arauto a Imortal, e a unidade, as estrelas). `None` fora da escala.
pub fn rank_label(tier: u32) -> Option<String> {
    let medal = match tier / 10 {
        1 => translate("Arauto"),
        2 => translate("Guardião"),
        3 => translate("Cruzado"),
        4 => translate("Arconte"),
        5 => translate("Lenda"),
        6 => translate("Ancestral"),
        7 => translate("Divino"),
        8 => return Some(translate("Imortal")),
        _ => return None,
    };
    match tier % 10 {
        stars @ 1..=5 => Some(format!("{medal} {stars}")),
        _ => Some(medal),
    }
}

/// Cabeçalho de uma vitória profissional: a liga e, no detalhe, a data e a
/// duração.
pub fn pro_win_heading(win: &ProWin) -> (String, String) {
    let recent = &win.hero_match;
    let league = recent
        .league_name
        .clone()
        .unwrap_or_else(|| translate("Liga sem nome"));
    let mut details = Vec::new();
    if let Some(start_time) = recent.start_time {
        details.push(format_date(start_time));
    }
    if let Some(duration) = recent.duration_secs {
        details.push(translate_with("duração {}", &[&format_duration(duration)]));
    }
    (
        translate_with("Vitória profissional: {}", &[&league]),
        details.join(" · "),
    )
}

/// Explicação de quantas vitórias profissionais apareceram.
pub fn examined_note(examined: usize) -> String {
    translate_with(
        "Entre as {} partidas de liga recentes examinadas; derrotas, outros patches e partidas sem build ficam de fora.",
        &[&examined.to_string()],
    )
}

/// Momento de compra no relógio da partida; antes do início, sem relógio.
pub fn purchase_time(seconds: i32) -> String {
    match u32::try_from(seconds) {
        Ok(seconds) => format_duration(seconds),
        Err(_) => translate("antes do início"),
    }
}

/// Linha da fonte, com o aviso de dados antigos ou ausentes.
pub fn source_row(source: &str, origin: Option<DataOrigin>) -> RowContent {
    let (level, freshness) = match origin {
        Some(DataOrigin::Live | DataOrigin::Cached) => (Level::Info, translate("Dados atuais")),
        Some(DataOrigin::Stale) => (
            Level::Warning,
            translate(
                "Dados antigos: o OpenDota não respondeu, então aparece o que estava guardado.",
            ),
        ),
        Some(DataOrigin::Embedded) | None => (Level::Warning, translate("Sem dados")),
    };
    RowContent::new(level, source, &freshness)
}

fn popularity_rows(
    recommendations: &Recommendations,
    items: &ItemCatalog,
    redactor: &Redactor,
) -> Vec<RowContent> {
    let popularity = &recommendations.popularity;
    let mut rows: Vec<RowContent> = popularity
        .value
        .iter()
        .flat_map(|value| &value.phases)
        .filter(|(_, counts)| !counts.is_empty())
        .map(|(phase, counts)| {
            let names: Vec<&str> = counts
                .iter()
                .take(POPULAR_ITEMS_SHOWN)
                .map(|(item, _)| items.display_name(item))
                .collect();
            RowContent::new(
                Level::Info,
                &translate_with("{}: {}", &[&phase_label(*phase), &names.join(", ")]),
                &translate("Popularidade em partidas profissionais, não taxa de vitória."),
            )
        })
        .collect();
    rows.push(source_row(
        &translate_with(
            "Fonte: OpenDota, popularidade em partidas profissionais · patch {}",
            &[&recommendations.patch.name],
        ),
        popularity.origin,
    ));
    if popularity.value.is_none()
        && let Some(error) = &popularity.error
    {
        rows.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    rows
}

pub fn phase_label(phase: PopularityPhase) -> String {
    match phase {
        PopularityPhase::Start => translate("Início"),
        PopularityPhase::Early => translate("Até 10 minutos"),
        PopularityPhase::Mid => translate("De 10 a 25 minutos"),
        PopularityPhase::Late => translate("Depois de 25 minutos"),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/items_tests.rs"]
mod tests;
