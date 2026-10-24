//! Envia um único evento sintético ao servidor local com a configuração do app.

use std::path::Path;
use std::process::ExitCode;

use dota_companion::application::error::{Diagnostic, Redactor, ToDiagnostic};
use dota_companion::application::ports::ConnectionTester;
use dota_companion::domain::gsi_config::GsiEndpoint;
use dota_companion::gsi::client::HttpConnectionTester;
use dota_companion::infrastructure::{config, logging};
use dota_companion::presentation::widgets::diagnostic_text;

fn main() -> ExitCode {
    logging::install_panic_hook();
    match run() {
        Ok(()) => {
            // Saída direta para o usuário da ferramenta, independente de RUST_LOG.
            println!("Evento de teste enviado com sucesso.");
            ExitCode::SUCCESS
        }
        Err(diagnostic) => {
            eprintln!("{}", diagnostic_text(&diagnostic));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<Diagnostic>> {
    let env_file = config::read_env_file(Path::new(config::ENV_FILE_NAME))
        .map_err(|error| error.to_diagnostic(&Redactor::default()))?;
    let config = config::load_or_init(&config::config_path(), |name| {
        config::lookup_variable(name, Some(&env_file))
    })
    .map_err(|error| error.to_diagnostic(&Redactor::default()))?;
    let endpoint = GsiEndpoint {
        port: config.gsi_port,
        token: config.gsi_token,
    };
    let redactor = Redactor::new([endpoint.token.as_str()]);
    let tester = HttpConnectionTester::new().map_err(|error| error.to_diagnostic(&redactor))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            Diagnostic::unclassified(
                dota_companion::application::error::UiText::new("Teste de conexão falhou"),
                "Não foi possível iniciar o runtime do teste de conexão.",
                &error,
                &redactor,
            )
        })?;
    runtime
        .block_on(tester.send_test_event(&endpoint))
        .map_err(|error| Box::new(error.to_diagnostic(&redactor)))
}
