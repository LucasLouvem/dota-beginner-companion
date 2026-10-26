//! Composição do aplicativo: cria os adaptadores, inicia o núcleo e o
//! servidor GSI em um runtime tokio e abre a janela GTK.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use relm4::RelmApp;
use tokio::sync::{mpsc, watch};

use dota_companion::application::core::{self, CoreDependencies};
use dota_companion::application::error::{Redactor, ToDiagnostic, log_error, log_warning};
use dota_companion::application::ports::{CacheStore, CallBudget, PublicBuildStore, StatsProvider};
use dota_companion::application::state::{AppSnapshot, GsiLogStatus};
use dota_companion::application::use_cases::call_budget::BudgetLimits;
use dota_companion::application::use_cases::external_sources::ExternalSources;
use dota_companion::application::use_cases::public_collection::run_collection;
use dota_companion::domain::gsi_config::GsiEndpoint;
use dota_companion::domain::hero_catalog::HeroCatalog;
use dota_companion::domain::item_catalog::ItemCatalog;
use dota_companion::infrastructure::{config, logging};
use dota_companion::presentation::app::{App, CoreHandle};
use dota_companion::presentation::widgets::diagnostic_text;
use dota_companion::providers::http_json::{HttpJsonClient, HttpJsonConfig};
use dota_companion::providers::opendota::OpenDotaProvider;
use dota_companion::storage::call_budget::SqliteCallBudget;
use dota_companion::storage::public_builds::SqlitePublicBuildStore;
use dota_companion::storage::sqlite_cache::SqliteCacheStore;
use dota_companion::{application, gsi, infrastructure, presentation, storage};

const APP_ID: &str = "io.github.dotacompanion.DotaBeginnerCompanion";

fn main() -> anyhow::Result<()> {
    logging::install_panic_hook();
    // Lido antes do log porque o `.env` pode definir o `RUST_LOG`.
    let env_file = config::read_env_file(Path::new(config::ENV_FILE_NAME));
    let env_values = env_file.as_ref().ok();
    // O ambiente do processo tem prioridade sobre o `.env`, como no dotenvy.
    let lookup = |name: &str| config::lookup_variable(name, env_values);

    let (log_filter, log_filter_error) = logging::log_filter(lookup("RUST_LOG").as_deref());
    tracing_subscriber::fmt().with_env_filter(log_filter).init();
    // Registrado como erro porque o filtro de fallback só deixa passar erros.
    if let Some(filter_error) = log_filter_error {
        log_error(
            &filter_error.to_diagnostic(&Redactor::default()),
            "invalid RUST_LOG; logging errors only",
        );
    }
    if let Err(env_error) = &env_file {
        return Err(startup_failure(env_error, &Redactor::default()));
    }
    if let Err(translation_error) = presentation::i18n::init() {
        log_warning(
            &translation_error.to_diagnostic(&Redactor::default()),
            "translations unavailable; using built-in Portuguese texts",
        );
    }

    let config_path = config::config_path();
    let config = config::load_or_init(&config_path, lookup)
        .map_err(|config_error| startup_failure(&config_error, &Redactor::default()))?;
    let endpoint = GsiEndpoint {
        port: config.gsi_port,
        token: config.gsi_token.clone(),
    };
    let redactor = Redactor::new([endpoint.token.as_str()]);
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();

    // Sem os dados embutidos o app segue: os itens aparecem com o nome interno
    // e montagens não são reconhecidas.
    let items = Arc::new(
        infrastructure::item_data::embedded_catalog().unwrap_or_else(|data_error| {
            log_error(
                &data_error.to_diagnostic(&Redactor::default()),
                "embedded item data is invalid; items keep their internal names",
            );
            ItemCatalog::default()
        }),
    );

    // Sem o banco local ou o cliente HTTP o app segue, sem recomendações nem
    // coleta de partidas públicas.
    let external = external_sources(&storage::database_path(), &items);
    let dependencies = CoreDependencies {
        locator: Arc::new(infrastructure::steam_locator::SteamLocator::new(
            home,
            config.extra_libraries.clone(),
        )),
        probe: Arc::new(infrastructure::process::ProcProbe::default()),
        files: Arc::new(infrastructure::config_files::LocalConfigFiles),
        tester: Arc::new(
            gsi::client::HttpConnectionTester::new()
                .map_err(|tester_error| startup_failure(&tester_error, &redactor))?,
        ),
        recorder: config.gsi_log.clone().map(|log_path| {
            Arc::new(infrastructure::gsi_log_file::JsonLinesGsiRecorder::new(
                log_path,
            )) as Arc<dyn application::ports::GsiRecorder>
        }),
        items: Arc::clone(&items),
        endpoint: endpoint.clone(),
        external: external.clone(),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (command_sender, command_receiver) = mpsc::unbounded_channel();
    let (event_sender, event_receiver) = mpsc::channel(256);
    let (state_sender, state_receiver) = watch::channel(AppSnapshot {
        config_path,
        gsi_log: config.gsi_log.clone().map(|path| GsiLogStatus {
            path,
            written: 0,
            last_error: None,
            stopped: None,
        }),
        redactor,
        ..AppSnapshot::default()
    });

    runtime.spawn(gsi::server::serve(
        endpoint.port,
        endpoint.token.into(),
        event_sender,
    ));
    runtime.spawn(core::run(
        dependencies,
        command_receiver,
        event_receiver,
        Arc::new(state_sender),
    ));
    if let Some(sources) = external {
        runtime.spawn(run_collection(sources));
    }

    // Sem os dados embutidos o app segue: a aba Herói mostra só o que o GSI informa.
    let heroes = infrastructure::hero_data::embedded_catalog().unwrap_or_else(|data_error| {
        log_error(
            &data_error.to_diagnostic(&Redactor::default()),
            "embedded hero data is invalid; hero tab shows GSI data only",
        );
        HeroCatalog::default()
    });

    // GTK recebe só o nome do programa; argumentos são nossos.
    RelmApp::new(APP_ID)
        .with_args(Vec::new())
        .run::<App>(CoreHandle {
            commands: command_sender,
            state: state_receiver,
            heroes: Arc::new(heroes),
            items,
        });
    runtime.shutdown_background();
    Ok(())
}

/// Fontes de dados externos (OpenDota, orçamento, cache e builds públicas),
/// ou `None` (com o erro no log) quando o banco local ou o cliente HTTP não
/// puderam ser abertos.
fn external_sources(database: &Path, items: &Arc<ItemCatalog>) -> Option<ExternalSources> {
    let failed = |diagnostic_source: &dyn ToDiagnostic| {
        log_error(
            &diagnostic_source.to_diagnostic(&Redactor::default()),
            "external data disabled: no recommendations or public match collection",
        );
    };
    let budget = SqliteCallBudget::open(database, BudgetLimits::default())
        .map_err(|storage_error| failed(&storage_error))
        .ok()?;
    let store = SqlitePublicBuildStore::open(database)
        .map_err(|storage_error| failed(&storage_error))
        .ok()?;
    let cache = SqliteCacheStore::open(database)
        .map_err(|storage_error| failed(&storage_error))
        .ok()?;
    let http = HttpJsonClient::new(HttpJsonConfig::opendota())
        .map_err(|client_error| failed(&client_error))
        .ok()?;
    let embedded_patches = infrastructure::patch_data::embedded_catalog()
        .map_err(|data_error| {
            log_warning(
                &data_error.to_diagnostic(&Redactor::default()),
                "embedded patches unavailable; the current patch needs OpenDota",
            );
        })
        .ok();
    Some(ExternalSources {
        provider: Arc::new(OpenDotaProvider::new(http, Arc::clone(items)))
            as Arc<dyn StatsProvider>,
        budget: Arc::new(budget) as Arc<dyn CallBudget>,
        store: Arc::new(store) as Arc<dyn PublicBuildStore>,
        cache: Arc::new(cache) as Arc<dyn CacheStore>,
        items: Arc::clone(items),
        embedded_patches,
    })
}

/// Falha que impede a inicialização. Só o diagnóstico sanitizado vai para o
/// terminal: o erro original não entra na cadeia do `anyhow`, porque ela é
/// impressa por inteiro e pode citar o conteúdo do `config.toml`.
fn startup_failure(error: &impl ToDiagnostic, redactor: &Redactor) -> anyhow::Error {
    anyhow::anyhow!("{}", diagnostic_text(&error.to_diagnostic(redactor)))
}
