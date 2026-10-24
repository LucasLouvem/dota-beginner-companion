//! Composição do aplicativo: cria os adaptadores, inicia o núcleo e o
//! servidor GSI em um runtime tokio e abre a janela GTK.

mod application;
mod domain;
mod gsi;
mod infrastructure;
mod presentation;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use relm4::RelmApp;
use tokio::sync::{mpsc, watch};

use application::core::{self, CoreDependencies};
use application::state::{AppSnapshot, GsiLogStatus};
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

    let dependencies = CoreDependencies {
        locator: Arc::new(infrastructure::steam_locator::SteamLocator::new(
            home,
            config.extra_libraries.clone(),
        )),
        probe: Arc::new(infrastructure::process::ProcProbe::default()),
        files: Arc::new(infrastructure::config_files::LocalConfigFiles),
        tester: Arc::new(gsi::client::HttpConnectionTester::new()),
        recorder: config.gsi_log.clone().map(|log_path| {
            Arc::new(infrastructure::gsi_log_file::JsonLinesGsiRecorder::new(
                log_path,
            )) as Arc<dyn application::ports::GsiRecorder>
        }),
        endpoint: endpoint.clone(),
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
        }),
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

    // GTK recebe só o nome do programa; argumentos são nossos.
    RelmApp::new(APP_ID)
        .with_args(Vec::new())
        .run::<App>(CoreHandle {
            commands: command_sender,
            state: state_receiver,
        });
    runtime.shutdown_background();
    Ok(())
}
