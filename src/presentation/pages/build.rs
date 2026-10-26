//! Conteúdo da página da build, sem widgets: o cabeçalho das recomendações,
//! a lista de vitórias para escolher e, para a vitória escolhida, os itens e
//! a ordem de habilidades acompanhados pelo estado atual da partida. Cada
//! parte depende só do que a muda: o cabeçalho e a lista não dependem da
//! seleção, e o progresso não refaz a lista.

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::state::{ModeState, RecommendationStatus};
use crate::application::use_cases::fallback::DataOrigin;
use crate::application::use_cases::pro_builds::PRO_WINS_SHOWN;
use crate::application::use_cases::recommendations::{
    BuildId, BuildSource, PUBLIC_WINS_SHOWN, Recommendations,
};
use crate::domain::ability_catalog::AbilityIdCatalog;
use crate::domain::build_progress::{
    ItemOrderEvidence, PurchaseWindow, StepState, ability_max_order, ability_progress,
    build_item_order, held_items, item_progress, points_spent, purchases_by_window, starting_items,
};
use crate::domain::builds::Purchase;
use crate::domain::game::{GameMode, GameSnapshot};
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::PopularityPhase;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{Level, RowContent, format_date, format_duration, format_time};

use super::hero::{mode_label, stat_lines};

/// Catálogos usados pelos nomes de heróis, itens e habilidades.
pub struct BuildCatalogs<'a> {
    pub heroes: &'a HeroCatalog,
    pub items: &'a ItemCatalog,
    pub abilities: &'a AbilityIdCatalog,
}

/// Linhas do cabeçalho: herói, modo e patch, contagem de vitórias, origem e
/// idade dos dados, avisos do modo e falhas das fontes. Não depende da build
/// escolhida.
pub fn header_rows(
    status: &RecommendationStatus,
    mode: &ModeState,
    heroes: &HeroCatalog,
    redactor: &Redactor,
) -> Vec<RowContent> {
    let RecommendationStatus::Ready(recommendations) = status else {
        return status_rows(status, heroes, redactor).unwrap_or_default();
    };
    let pro = &recommendations.pro;
    let mut rows = vec![ready_header(recommendations, heroes)];
    rows.push(RowContent::new(
        Level::Info,
        &translate_with(
            "{} vitórias profissionais · {} vitórias públicas",
            &[
                &pro.wins.len().to_string(),
                &recommendations.public_wins.len().to_string(),
            ],
        ),
        &translate_with(
            "Fonte: OpenDota, patch {}. {} partidas de liga recentes examinadas; as públicas vêm da coleta em segundo plano.",
            &[&recommendations.patch.name, &pro.examined.to_string()],
        ),
    ));
    if pro.wins.len() < PRO_WINS_SHOWN || recommendations.public_wins.len() < PUBLIC_WINS_SHOWN {
        rows.push(RowContent::new(
            Level::Info,
            &translate("Poucas vitórias encontradas"),
            &translate_with(
                "A página mostra até {} vitórias profissionais e {} públicas. Derrotas, outros patches e partidas sem build ficam de fora, e a coleta pública continua enquanto o aplicativo está aberto.",
                &[&PRO_WINS_SHOWN.to_string(), &PUBLIC_WINS_SHOWN.to_string()],
            ),
        ));
    }
    if pro.candidates_origin == Some(DataOrigin::Stale) {
        let since = pro
            .candidates_fetched_at
            .map(|fetched_at| translate_with(" de {}", &[&format_time(fetched_at)]))
            .unwrap_or_default();
        rows.push(RowContent::new(
            Level::Warning,
            &translate_with("Dados profissionais antigos{}", &[&since]),
            &translate(
                "O OpenDota não respondeu, então aparece a lista guardada na última consulta.",
            ),
        ));
    }
    rows.extend(mode_notes(recommendations.key.mode, mode));
    if let Some(error) = &pro.error {
        rows.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    if let Some(error) = &recommendations.public_error {
        rows.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    rows
}

/// Avisos do modo: modo ainda não informado, Turbo (vitórias profissionais
/// não são de Turbo) e Contra bots (dados de Escolha Livre).
fn mode_notes(mode: GameMode, state: &ModeState) -> Vec<RowContent> {
    let mut notes = Vec::new();
    if !matches!(state, ModeState::Confirmed { .. }) {
        notes.push(RowContent::new(
            Level::Info,
            &translate_with(
                "Modo ainda não informado; mostrando dados de {}",
                &[&mode_label(mode)],
            ),
            &translate("O modo é perguntado quando a partida é encontrada e pode ser alterado logo abaixo."),
        ));
    }
    match mode {
        GameMode::Turbo => notes.push(turbo_warning(&translate(
            "As vitórias públicas são de Turbo; as profissionais são partidas normais, com outro ritmo.",
        ))),
        GameMode::Bots => notes.push(bots_note()),
        GameMode::AllPick => {}
    }
    notes
}

/// Uma vitória na lista lateral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntry {
    pub id: BuildId,
    pub title: String,
    pub subtitle: String,
    /// Itens finais (inventário), para as miniaturas.
    pub items: Vec<ItemChip>,
}

/// Lista lateral: as vitórias profissionais e as públicas, na ordem das
/// recomendações. Não depende da build escolhida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sidebar {
    pub pro: Vec<BuildEntry>,
    pub public: Vec<BuildEntry>,
    /// Explicação quando não há nenhuma vitória.
    pub empty: Option<RowContent>,
}

impl Sidebar {
    /// Todas as entradas, na ordem da lista.
    pub fn entries(&self) -> impl Iterator<Item = &BuildEntry> {
        self.pro.iter().chain(&self.public)
    }
}

pub fn sidebar(status: &RecommendationStatus, items: &ItemCatalog) -> Sidebar {
    let RecommendationStatus::Ready(recommendations) = status else {
        return Sidebar {
            empty: Some(RowContent::new(
                Level::Info,
                &translate("Nenhuma build para escolher"),
                &translate(
                    "As vitórias aparecem quando o Dota informa o herói escolhido e as recomendações ficam prontas.",
                ),
            )),
            ..Sidebar::default()
        };
    };
    let pro: Vec<BuildEntry> = recommendations
        .pro
        .wins
        .iter()
        .map(|win| {
            let recent = &win.hero_match;
            let mut details = Vec::new();
            if let Some(start_time) = recent.start_time {
                details.push(format_date(start_time));
            }
            if let Some(duration) = recent.duration_secs {
                details.push(format_duration(duration));
            }
            BuildEntry {
                id: BuildId {
                    source: BuildSource::Pro,
                    match_id: recent.match_id,
                },
                title: recent
                    .league_name
                    .clone()
                    .unwrap_or_else(|| translate("Liga sem nome")),
                subtitle: details.join(" · "),
                items: plain_chips(&win.build.inventory, items),
            }
        })
        .collect();
    let public: Vec<BuildEntry> = recommendations
        .public_wins
        .iter()
        .map(|win| BuildEntry {
            id: BuildId {
                source: BuildSource::Public,
                match_id: win.match_id,
            },
            title: win
                .avg_rank_tier
                .and_then(rank_label)
                .map(|rank| translate_with("Partida pública · {}", &[&rank]))
                .unwrap_or_else(|| translate("Partida pública")),
            subtitle: win.start_time.map(format_date).unwrap_or_default(),
            items: plain_chips(&win.build.inventory, items),
        })
        .collect();
    let empty = (pro.is_empty() && public.is_empty()).then(|| {
        RowContent::new(
            Level::Info,
            &translate("Nenhuma vitória recente encontrada"),
            &translate_with(
                "{} partidas de liga recentes foram examinadas; derrotas, outros patches e partidas sem build ficam de fora. As vitórias públicas aparecem conforme a coleta em segundo plano avança.",
                &[&recommendations.pro.examined.to_string()],
            ),
        )
    });
    Sidebar { pro, public, empty }
}

/// Uma linha da lista lateral: título de grupo, vitória ou explicação de
/// lista vazia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarItem {
    Group(String),
    Entry(BuildEntry),
    Empty(RowContent),
}

/// Linhas da lista lateral, com o título de cada grupo antes das vitórias.
pub fn sidebar_items(sidebar: &Sidebar) -> Vec<SidebarItem> {
    let mut items = Vec::new();
    for (title, entries) in [
        (translate("Vitórias profissionais"), &sidebar.pro),
        (translate("Vitórias públicas"), &sidebar.public),
    ] {
        if !entries.is_empty() {
            items.push(SidebarItem::Group(title));
            items.extend(entries.iter().cloned().map(SidebarItem::Entry));
        }
    }
    items.extend(sidebar.empty.iter().cloned().map(SidebarItem::Empty));
    items
}

/// Build da linha `index` da lista; `None` para título, explicação ou
/// posição fora da lista.
pub fn entry_at(items: &[SidebarItem], index: i32) -> Option<BuildId> {
    let index = usize::try_from(index).ok()?;
    match items.get(index)? {
        SidebarItem::Entry(entry) => Some(entry.id),
        SidebarItem::Group(_) | SidebarItem::Empty(_) => None,
    }
}

/// Posição da linha da build `id` na lista.
pub fn index_of(items: &[SidebarItem], id: &BuildId) -> Option<usize> {
    items
        .iter()
        .position(|item| matches!(item, SidebarItem::Entry(entry) if entry.id == *id))
}

/// Um item numa faixa de itens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemChip {
    /// Nome interno, que também identifica a imagem.
    pub item: String,
    pub label: String,
    /// Segundos de partida da compra (negativo antes do início), quando a
    /// faixa vem do registro de compras.
    pub time: Option<i32>,
    /// Situação na partida atual, nas faixas que acompanham o progresso.
    pub state: Option<StepState>,
}

impl ItemChip {
    /// Texto para o leitor de tela: nome, minuto e situação.
    pub fn accessible_label(&self) -> String {
        let mut parts = vec![self.label.clone()];
        match self.time {
            Some(time) if time < 0 => parts.push(translate("comprado antes do início")),
            Some(time) => parts.push(translate_with("comprado em {}", &[&purchase_time(time)])),
            None => {}
        }
        if let Some(state) = self.state {
            parts.push(step_label(state));
        }
        parts.join(", ")
    }

    /// Símbolo da situação, para não depender só da cor.
    pub fn marker(&self) -> &'static str {
        match self.state {
            Some(StepState::Done) => "✓",
            Some(StepState::Next) => "→",
            Some(StepState::Pending) | None => "",
        }
    }
}

/// Itens sem minuto nem situação, só com o nome de exibição.
pub fn plain_chips(names: &[String], items: &ItemCatalog) -> Vec<ItemChip> {
    names
        .iter()
        .map(|item| ItemChip {
            item: item.clone(),
            label: items.display_name(item).to_owned(),
            time: None,
            state: None,
        })
        .collect()
}

pub fn step_label(state: StepState) -> String {
    match state {
        StepState::Done => translate("já feito"),
        StepState::Next => translate("próximo"),
        StepState::Pending => translate("ainda não"),
    }
}

/// Grade da ordem de habilidades: uma linha por habilidade (os talentos
/// juntos numa linha), uma coluna por ponto gasto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbilityGrid {
    pub rows: Vec<String>,
    /// Para cada ponto, a linha da habilidade e a situação.
    pub points: Vec<(usize, StepState)>,
    /// Ordem em que cada habilidade recebeu o último ponto.
    pub max_order: String,
    /// Ordem completa em texto, para o leitor de tela.
    pub summary: String,
}

/// Conteúdo da build escolhida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildDetail {
    pub notes: Vec<RowContent>,
    pub starting: Vec<ItemChip>,
    pub order: Vec<ItemChip>,
    pub finals: Vec<ItemChip>,
    pub backpack: Vec<ItemChip>,
    pub neutral: Vec<ItemChip>,
    /// Todas as compras por janela de tempo.
    pub windows: Vec<(String, Vec<ItemChip>)>,
    pub abilities: Option<AbilityGrid>,
    /// Explicação quando não há ordem de habilidades.
    pub abilities_note: Option<RowContent>,
}

/// Conteúdo da build `id` acompanhado pelo estado `game`; `None` quando a
/// build não está nas recomendações.
pub fn build_detail(
    recommendations: &Recommendations,
    id: &BuildId,
    game: Option<&GameSnapshot>,
    catalogs: &BuildCatalogs<'_>,
) -> Option<BuildDetail> {
    let build = recommendations.build(id)?;
    let items = catalogs.items;
    let chip = |item: &str| ItemChip {
        item: item.to_owned(),
        label: items.display_name(item).to_owned(),
        time: None,
        state: None,
    };
    let bought = |purchase: &Purchase| ItemChip {
        time: Some(purchase.time),
        ..chip(&purchase.item)
    };

    let mut notes = Vec::new();
    if id.source == BuildSource::Pro && recommendations.key.mode == GameMode::Turbo {
        notes.push(turbo_warning(&translate(
            "Esta vitória é de uma partida normal: os minutos de compra não valem para o ritmo do Turbo.",
        )));
    }
    let order = build_item_order(build);
    if order.evidence == ItemOrderEvidence::InventorySlots {
        notes.push(RowContent::new(
            Level::Info,
            &translate("Partida sem registro de compras"),
            &translate("O OpenDota não analisou esta partida: a ordem abaixo é a dos espaços do inventário, não a de compra, e não há itens iniciais nem a lista completa de compras."),
        ));
    }
    let held = game.map(|game| held_items(game, items)).unwrap_or_default();
    let order_chips = item_progress(&order.items, &held)
        .into_iter()
        .map(|step| ItemChip {
            state: Some(step.state),
            ..chip(&step.item)
        })
        .collect();

    let neutral = build
        .neutral
        .iter()
        .chain(&build.neutral_enhancement)
        .map(|item| chip(item))
        .collect();
    let windows = purchases_by_window(build)
        .into_iter()
        .map(|(window, purchases)| (window_label(window), purchases.iter().map(bought).collect()))
        .collect();

    let (abilities, abilities_note) = match recommendations.ability_upgrades(id) {
        Some(upgrades) if !upgrades.is_empty() => (
            Some(ability_grid(
                upgrades,
                game.and_then(points_spent),
                catalogs,
            )),
            None,
        ),
        _ => (
            None,
            Some(RowContent::new(
                Level::Info,
                &translate("Sem ordem de habilidades"),
                &match id.source {
                    BuildSource::Pro => translate(
                        "O OpenDota não analisou esta partida, então a ordem de habilidades não está disponível.",
                    ),
                    BuildSource::Public => translate(
                        "As partidas públicas guardadas trazem só os itens; escolha uma vitória profissional para ver a ordem de habilidades.",
                    ),
                },
            )),
        ),
    };

    Some(BuildDetail {
        notes,
        starting: starting_items(build).iter().map(bought).collect(),
        order: order_chips,
        finals: build.inventory.iter().map(|item| chip(item)).collect(),
        backpack: build.backpack.iter().map(|item| chip(item)).collect(),
        neutral,
        windows,
        abilities,
        abilities_note,
    })
}

/// Grade da ordem real de habilidades, marcada pelos pontos já gastos.
pub fn ability_grid(
    upgrades: &[u32],
    spent: Option<usize>,
    catalogs: &BuildCatalogs<'_>,
) -> AbilityGrid {
    let talents = translate("Talentos");
    let name_of = |id: u32| {
        if catalogs.abilities.is_talent(id) {
            talents.clone()
        } else {
            ability_name(id, catalogs.heroes, catalogs.abilities)
        }
    };
    let mut rows: Vec<String> = Vec::new();
    let points = ability_progress(upgrades, spent)
        .into_iter()
        .map(|(id, state)| {
            let name = name_of(id);
            let row = rows.iter().position(|row| *row == name).unwrap_or_else(|| {
                rows.push(name);
                rows.len() - 1
            });
            (row, state)
        })
        .collect();
    let max_order = ability_max_order(upgrades, |id| catalogs.abilities.is_talent(id))
        .into_iter()
        .map(name_of)
        .collect::<Vec<_>>()
        .join(" > ");
    let summary = upgrades
        .iter()
        .enumerate()
        .map(|(index, id)| {
            translate_with("ponto {}: {}", &[&(index + 1).to_string(), &name_of(*id)])
        })
        .collect::<Vec<_>>()
        .join("; ");
    AbilityGrid {
        rows,
        points,
        max_order,
        summary,
    }
}

pub fn window_label(window: PurchaseWindow) -> String {
    match window {
        PurchaseWindow::BeforeStart => translate("Antes do início"),
        PurchaseWindow::Early => translate("Até 10 minutos"),
        PurchaseWindow::Mid => translate("De 10 a 25 minutos"),
        PurchaseWindow::Late => translate("Depois de 25 minutos"),
    }
}

/// Faixa curta com o progresso da partida atual (nível, abates e ouro), sem
/// a linha do tempo, que fica mais abaixo.
pub fn progress_row(game: Option<&GameSnapshot>) -> RowContent {
    match game {
        Some(game) if game.hero_level.is_some() => RowContent::new(
            Level::Info,
            &translate("Partida atual"),
            &stat_lines(&game.stats, game.hero_level).join(" · "),
        ),
        _ => RowContent::new(
            Level::Info,
            &translate("Partida atual"),
            &translate("O progresso aparece quando o Dota envia os dados do herói."),
        ),
    }
}

/// Itens mostrados por fase na popularidade.
const POPULAR_ITEMS_SHOWN: usize = 5;

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

/// Itens mais comprados em cada fase, em partidas profissionais, com a
/// fonte; mostra popularidade, não uma build.
pub fn popularity_rows(
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

/// Nome de exibição da habilidade; talentos aparecem como "Talento", e um
/// ID desconhecido, como o próprio número.
pub fn ability_name(id: u32, heroes: &HeroCatalog, abilities: &AbilityIdCatalog) -> String {
    match abilities.name(id) {
        Some(name) if name.starts_with("special_bonus") => translate("Talento"),
        Some(name) => heroes.ability_display_name(name).unwrap_or(name).to_owned(),
        None => translate_with("habilidade {}", &[&id.to_string()]),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/presentation/pages/build_tests.rs"]
mod tests;
