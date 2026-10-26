//! Monta o núcleo com fakes dos ports e espera mudanças no `AppSnapshot`.
//! Fica aqui, e não em `tests/support/`, porque importa o crate como
//! `dota_companion`, enquanto `tests/support/` usa os caminhos `crate::`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dota_companion::application::core::{self, CoreDependencies};
use dota_companion::application::event_bus::{CoreEvent, UiCommand};
use dota_companion::application::ports::{
    ConfigFiles, ConnectionTester, DotaLocator, LocatorResult, ProcessProbe,
};
use dota_companion::application::state::AppSnapshot;
use dota_companion::domain::dota_install::DotaInstall;
use dota_companion::domain::gsi_config::GsiEndpoint;
use dota_companion::gsi::client::HttpConnectionTester;
use dota_companion::infrastructure::config_files::LocalConfigFiles;
use tokio::sync::{mpsc, watch};

/// Tempo máximo de espera por uma mudança de estado.
pub const STATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Localizador com uma única biblioteca Steam, opcionalmente com o Dota.
pub struct FixedLocator {
    pub library: Option<PathBuf>,
}

impl DotaLocator for FixedLocator {
    fn locate(&self) -> LocatorResult {
        let Some(library) = &self.library else {
            return LocatorResult {
                searched_libraries: Vec::new(),
                installs: Vec::new(),
            };
        };
        LocatorResult {
            searched_libraries: vec![library.clone()],
            installs: vec![DotaInstall {
                library: library.clone(),
                game_root: DotaInstall::game_root_for(library),
                native_binary: true,
                proton_prefix: false,
            }],
        }
    }
}

/// Informa sempre que o Dota está fechado.
pub struct ClosedDota;

impl ProcessProbe for ClosedDota {
    fn dota_running(&self) -> bool {
        false
    }
}

/// Núcleo em execução e os canais que a interface e o servidor usariam.
pub struct RunningCore {
    pub commands: mpsc::UnboundedSender<UiCommand>,
    pub events: mpsc::Sender<CoreEvent>,
    pub state: watch::Receiver<AppSnapshot>,
}

impl RunningCore {
    /// Inicia `core::run` com arquivos reais e o testador HTTP real.
    pub fn start(endpoint: GsiEndpoint, library: Option<&Path>) -> Self {
        let dependencies = CoreDependencies {
            locator: Arc::new(FixedLocator {
                library: library.map(Path::to_path_buf),
            }) as Arc<dyn DotaLocator>,
            probe: Arc::new(ClosedDota) as Arc<dyn ProcessProbe>,
            files: Arc::new(LocalConfigFiles) as Arc<dyn ConfigFiles>,
            tester: Arc::new(HttpConnectionTester::new().unwrap()) as Arc<dyn ConnectionTester>,
            recorder: None,
            items: Arc::new(dota_companion::infrastructure::item_data::embedded_catalog().unwrap()),
            endpoint,
            external: None,
            icons: None,
        };
        let (commands, command_receiver) = mpsc::unbounded_channel();
        let (events, event_receiver) = mpsc::channel(16);
        let (state_sender, state) = watch::channel(AppSnapshot::default());
        tokio::spawn(core::run(
            dependencies,
            command_receiver,
            event_receiver,
            Arc::new(state_sender),
        ));
        Self {
            commands,
            events,
            state,
        }
    }

    /// Espera até o estado satisfazer `condition` e devolve uma cópia dele.
    pub async fn wait_for(&mut self, condition: impl FnMut(&AppSnapshot) -> bool) -> AppSnapshot {
        tokio::time::timeout(STATE_TIMEOUT, self.state.wait_for(condition))
            .await
            .expect("o estado esperado deve ser publicado a tempo")
            .expect("o núcleo deve continuar rodando")
            .clone()
    }
}
