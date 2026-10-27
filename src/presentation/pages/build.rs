//! Conteúdo da página da build, sem widgets: o cabeçalho das recomendações,
//! a lista de vitórias para escolher e, para a vitória escolhida, os itens e
//! a ordem de habilidades acompanhados pelo estado atual da partida. Cada
//! parte depende só do que a muda: o cabeçalho e a lista não dependem da
//! seleção, e o progresso não refaz a lista.

use crate::application::error::{Redactor, ToDiagnostic};
use crate::application::ports::PublicWin;
use crate::application::state::RecommendationStatus;
use crate::application::use_cases::fallback::DataOrigin;
use crate::application::use_cases::pro_builds::{PRO_WINS_SHOWN, ProWin};
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::application::use_cases::recommendations::{
    BuildId, BuildSource, PUBLIC_WINS_SHOWN, Recommendations,
};
use crate::domain::ability_catalog::AbilityIdCatalog;
use crate::domain::build_progress::{
    ItemOrderEvidence, StepState, ability_progress, build_item_order, held_items, item_progress,
    points_spent, starting_items,
};
use crate::domain::game::{GameMode, GameSnapshot};
use crate::domain::hero_catalog::HeroCatalog;
use crate::domain::item_catalog::ItemCatalog;
use crate::domain::match_data::Kda;
use crate::presentation::i18n::{translate, translate_with};
use crate::presentation::widgets::{Level, RowContent, format_date, format_duration, format_time};

use super::build_compare::{Comparison, comparison};
use super::hero::mode_label;

/// Catálogos usados pelos nomes de heróis, itens e habilidades.
pub struct BuildCatalogs<'a> {
    pub heroes: &'a HeroCatalog,
    pub items: &'a ItemCatalog,
    pub abilities: &'a AbilityIdCatalog,
}

/// Cabeçalho da página: o título em destaque, detalhes secundários (fonte,
/// contagem e horário da consulta) e os avisos. Não depende da build
/// escolhida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderContent {
    pub title: String,
    pub details: String,
    /// Avisos e falhas: poucas vitórias, dados antigos, modo e erros.
    pub notices: Vec<RowContent>,
}

pub fn header_content(
    status: &RecommendationStatus,
    heroes: &HeroCatalog,
    redactor: &Redactor,
) -> HeaderContent {
    let RecommendationStatus::Ready(recommendations) = status else {
        return HeaderContent {
            title: translate("Build"),
            details: String::new(),
            notices: status_rows(status, heroes, redactor).unwrap_or_default(),
        };
    };
    let pro = &recommendations.pro;
    let summary = ready_header(recommendations, heroes);
    let details = [
        translate_with(
            "{} vitórias profissionais e {} públicas",
            &[
                &pro.wins.len().to_string(),
                &recommendations.public_wins.len().to_string(),
            ],
        ),
        translate_with(
            "fonte: OpenDota, {} partidas de liga recentes examinadas",
            &[&pro.examined.to_string()],
        ),
        summary.subtitle.clone(),
    ]
    .join(" · ");
    let mut notices = Vec::new();
    if pro.wins.len() < PRO_WINS_SHOWN || recommendations.public_wins.len() < PUBLIC_WINS_SHOWN {
        notices.push(RowContent::new(
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
        notices.push(RowContent::new(
            Level::Warning,
            &translate_with("Dados profissionais antigos{}", &[&since]),
            &translate(
                "O OpenDota não respondeu, então aparece a lista guardada na última consulta.",
            ),
        ));
    }
    notices.extend(mode_notes(recommendations.key.mode));
    if let Some(error) = &pro.error {
        notices.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    if let Some(error) = &recommendations.public_error {
        notices.push(RowContent::diagnostic(&error.to_diagnostic(redactor)));
    }
    HeaderContent {
        title: summary.title,
        details,
        notices,
    }
}

/// Avisos do modo: Contra bots (dados de Escolha Livre). O aviso de Turbo é
/// da build profissional escolhida, não da página, e o modo ainda não
/// informado aparece no card do modo.
fn mode_notes(mode: GameMode) -> Vec<RowContent> {
    match mode {
        GameMode::Bots => vec![bots_note()],
        GameMode::AllPick | GameMode::Turbo => Vec::new(),
    }
}

/// Uma vitória na lista lateral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntry {
    pub id: BuildId,
    pub title: String,
    pub subtitle: String,
    /// Abates, mortes e assistências do jogador nesta vitória.
    pub kda: Option<Kda>,
}

/// K/D/A curto, como "3/8/16".
pub fn kda_text(kda: Kda) -> String {
    format!("{}/{}/{}", kda.kills, kda.deaths, kda.assists)
}

/// K/D/A por extenso, para o leitor de tela.
pub fn kda_label(kda: Kda) -> String {
    translate_with(
        "abates {}, mortes {}, assistências {}",
        &[
            &kda.kills.to_string(),
            &kda.deaths.to_string(),
            &kda.assists.to_string(),
        ],
    )
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

pub fn sidebar(status: &RecommendationStatus) -> Sidebar {
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
    let pro: Vec<BuildEntry> = recommendations.pro.wins.iter().map(pro_entry).collect();
    let public: Vec<BuildEntry> = recommendations
        .public_wins
        .iter()
        .map(public_entry)
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

/// Entrada de uma vitória profissional: a liga, a data e a duração.
fn pro_entry(win: &ProWin) -> BuildEntry {
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
        kda: recent.kda,
    }
}

/// Entrada de uma vitória pública: o rank médio, no título (a lista já está
/// no grupo de vitórias públicas), e a data.
fn public_entry(win: &PublicWin) -> BuildEntry {
    BuildEntry {
        id: BuildId {
            source: BuildSource::Public,
            match_id: win.match_id,
        },
        title: win
            .avg_rank_tier
            .and_then(rank_label)
            .map(|rank| translate_with("Rank médio {}", &[&rank]))
            .unwrap_or_else(|| translate("Rank médio não informado")),
        subtitle: win.start_time.map(format_date).unwrap_or_default(),
        kda: win.kda,
    }
}

/// Build a mostrar depois de novas recomendações: a escolhida antes, se a
/// consulta é a mesma e ela ainda está na lista; senão, a primeira da lista.
/// `None` sem builds.
pub fn select_build(
    previous: Option<&(RecommendationKey, BuildId)>,
    key: &RecommendationKey,
    items: &[SidebarItem],
) -> Option<BuildId> {
    let kept = previous
        .filter(|(previous_key, id)| previous_key == key && index_of(items, id).is_some())
        .map(|(_, id)| *id);
    kept.or_else(|| first_entry(items))
}

/// Primeira build da lista, se houver.
pub fn first_entry(items: &[SidebarItem]) -> Option<BuildId> {
    items.iter().find_map(|item| match item {
        SidebarItem::Entry(entry) => Some(entry.id),
        _ => None,
    })
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
    /// Nome interno da habilidade de cada linha, que identifica a imagem;
    /// `None` nos talentos e nas habilidades que o catálogo não conhece.
    pub images: Vec<Option<String>>,
    /// Para cada ponto, a linha da habilidade e a situação.
    pub points: Vec<(usize, StepState)>,
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
    /// Aprimoramento do item neutro.
    pub enhancement: Vec<ItemChip>,
    pub abilities: Option<AbilityGrid>,
    /// Explicação quando não há ordem de habilidades.
    pub abilities_note: Option<RowContent>,
    /// Você × build; só as vitórias profissionais têm os totais finais.
    pub comparison: Option<Comparison>,
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

    let mut notes = Vec::new();
    // Só a build profissional escolhida em Turbo recebe o aviso: as públicas
    // dessa lista são de Turbo de verdade.
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

    let neutral = build.neutral.iter().map(|item| chip(item)).collect();
    let enhancement = build
        .neutral_enhancement
        .iter()
        .map(|item| chip(item))
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
        // O título do card já diz que são os itens de antes do início.
        starting: starting_items(build)
            .iter()
            .map(|purchase| chip(&purchase.item))
            .collect(),
        order: order_chips,
        finals: build.inventory.iter().map(|item| chip(item)).collect(),
        backpack: build.backpack.iter().map(|item| chip(item)).collect(),
        neutral,
        enhancement,
        abilities,
        abilities_note,
        comparison: recommendations
            .pro_stats(id)
            .and_then(|(stats, duration)| comparison(stats, duration, game)),
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
    let image_of = |id: u32| {
        (!catalogs.abilities.is_talent(id))
            .then(|| catalogs.abilities.name(id).map(str::to_owned))
            .flatten()
    };
    let mut rows: Vec<String> = Vec::new();
    let mut images: Vec<Option<String>> = Vec::new();
    let points = ability_progress(upgrades, spent)
        .into_iter()
        .map(|(id, state)| {
            let name = name_of(id);
            let row = rows.iter().position(|row| *row == name).unwrap_or_else(|| {
                rows.push(name);
                images.push(image_of(id));
                rows.len() - 1
            });
            (row, state)
        })
        .collect();
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
        images,
        points,
        summary,
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

/// Aviso da build profissional escolhida quando o modo é Turbo.
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
