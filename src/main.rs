//! Composição do aplicativo: cria os adaptadores, inicia o núcleo e o
//! servidor GSI em um runtime tokio e abre a janela GTK.

mod application;
mod domain;
mod gsi;
mod infrastructure;
mod presentation;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use relm4::RelmApp;
use tokio::sync::{mpsc, watch};

use application::core::{self, CoreDeps};
use application::state::AppSnapshot;
use domain::gsi_config::GsiEndpoint;
use infrastructure::config;
use presentation::app::{App, CoreHandle};

const APP_ID: &str = "io.github.dotacompanion.DotaBeginnerCompanion";

fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    presentation::i18n::init();

    let config_path = config::config_path();
    let config = config::load_or_init(&config_path)
        .with_context(|| format!("lendo {}", config_path.display()))?;
    let endpoint = GsiEndpoint {
        port: config.gsi_port,
        token: config.gsi_token.clone(),
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();

    let deps = CoreDeps {
        locator: Arc::new(infrastructure::steam_locator::SteamLocator::new(
            home,
            config.extra_libraries.clone(),
        )),
        probe: Arc::new(infrastructure::process::ProcProbe::default()),
        files: Arc::new(infrastructure::config_files::LocalConfigFiles),
        tester: Arc::new(gsi::client::HttpConnectionTester::new()),
        endpoint: endpoint.clone(),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::channel(256);
    let (state_tx, state_rx) = watch::channel(AppSnapshot {
        config_path,
        ..AppSnapshot::default()
    });

    runtime.spawn(gsi::server::serve(
        endpoint.port,
        endpoint.token.into(),
        event_tx,
    ));
    runtime.spawn(core::run(deps, command_rx, event_rx, Arc::new(state_tx)));

    // GTK recebe só o nome do programa; argumentos são nossos.
    RelmApp::new(APP_ID)
        .with_args(Vec::new())
        .run::<App>(CoreHandle {
            commands: command_tx,
            state: state_rx,
        });
    runtime.shutdown_background();
    Ok(())
}
