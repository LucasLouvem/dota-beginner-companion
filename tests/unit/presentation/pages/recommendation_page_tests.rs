//! Testes unitários de `src/presentation/pages/recommendation_page.rs`.

use super::*;
use crate::application::use_cases::fallback::DataOrigin;
use crate::application::use_cases::recommendation_request::RecommendationKey;
use crate::domain::game::GameMode;
use crate::test_support::{pro_builds, recommendations};

fn catalogs() -> RecommendationCatalogs {
    RecommendationCatalogs {
        heroes: Arc::default(),
        items: Arc::default(),
        abilities: Arc::default(),
    }
}

/// Um estado de cada tipo, exceto a interrupção, que precisa de erro real.
fn statuses() -> Vec<RecommendationStatus> {
    vec![
        RecommendationStatus::Unavailable,
        RecommendationStatus::Idle,
        RecommendationStatus::Loading {
            key: RecommendationKey {
                hero_id: 22,
                mode: GameMode::Turbo,
                patch_id: 60,
            },
        },
        RecommendationStatus::Ready(Arc::new(recommendations(
            GameMode::AllPick,
            pro_builds(vec![], 0, Some(DataOrigin::Live)),
            vec![],
        ))),
    ]
}

#[test]
fn every_tab_fills_each_of_its_sections_in_every_state() {
    for tab in [
        RecommendationTab::Items,
        RecommendationTab::Abilities,
        RecommendationTab::ProMatches,
    ] {
        for status in statuses() {
            let content = tab_content(tab, &status, &catalogs(), &Redactor::default());
            assert_eq!(
                content.len(),
                tab.section_titles().len(),
                "{tab:?} em {status:?}"
            );
            assert!(
                !content[0].is_empty(),
                "{tab:?} tem cabeçalho em {status:?}"
            );
        }
    }
}

#[test]
fn tabs_have_their_titles() {
    assert_eq!(RecommendationTab::Items.title(), "Itens");
    assert_eq!(RecommendationTab::Abilities.title(), "Habilidades");
    assert_eq!(RecommendationTab::ProMatches.title(), "Dados profissionais");
}
