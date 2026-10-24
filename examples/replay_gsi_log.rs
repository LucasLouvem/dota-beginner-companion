//! Reenvia ao servidor local, como se fosse o Dota, os eventos gravados pelo
//! log de desenvolvimento (`DOTA_COMPANION_GSI_LOG`). Serve para validar o
//! aplicativo sem o jogo instalado. Não imprime payloads: o log tem dados
//! pessoais.
//!
//! Uso: `cargo run --example replay_gsi_log -- [arquivo] [--interval-ms N]`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use dota_companion::application::error::{Diagnostic, Redactor, ToDiagnostic, UiText};
use dota_companion::domain::gsi_config::GsiEndpoint;
use dota_companion::infrastructure::{config, logging};
use dota_companion::presentation::widgets::diagnostic_text;
use serde_json::Value;

/// Arquivo usado quando nenhum é informado.
const DEFAULT_LOG_PATH: &str = "logs/gsi.jsonl";
/// Intervalo padrão entre eventos; o Dota envia vários por segundo.
const DEFAULT_INTERVAL_MS: u64 = 100;

fn main() -> ExitCode {
    logging::install_panic_hook();
    let arguments = match ReplayArguments::parse(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(usage_error) => {
            eprintln!("{usage_error}");
            eprintln!("Uso: replay_gsi_log [arquivo] [--interval-ms N]");
            return ExitCode::FAILURE;
        }
    };
    match run(&arguments) {
        Ok(summary) => {
            // Saída direta para o usuário da ferramenta, independente de RUST_LOG.
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(diagnostic) => {
            eprintln!("{}", diagnostic_text(&diagnostic));
            ExitCode::FAILURE
        }
    }
}

struct ReplayArguments {
    log_path: PathBuf,
    interval: Duration,
}

impl ReplayArguments {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut log_path = PathBuf::from(DEFAULT_LOG_PATH);
        let mut interval_ms = DEFAULT_INTERVAL_MS;
        while let Some(argument) = arguments.next() {
            if argument == "--interval-ms" {
                let value = arguments
                    .next()
                    .ok_or("--interval-ms precisa de um número")?;
                interval_ms = value
                    .parse()
                    .map_err(|_| format!("--interval-ms inválido: {value}"))?;
            } else {
                log_path = PathBuf::from(argument);
            }
        }
        Ok(Self {
            log_path,
            interval: Duration::from_millis(interval_ms),
        })
    }
}

fn run(arguments: &ReplayArguments) -> Result<String, Box<Diagnostic>> {
    let title = UiText::new("Reenvio do log GSI falhou");
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
    let log = std::fs::read_to_string(&arguments.log_path).map_err(|error| {
        Diagnostic::unclassified(
            title.clone(),
            format!("Não foi possível ler {}.", arguments.log_path.display()),
            &error,
            &redactor,
        )
    })?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            Diagnostic::unclassified(
                title.clone(),
                "Não foi possível iniciar o runtime do reenvio.",
                &error,
                &redactor,
            )
        })?;
    runtime.block_on(replay(&log, &endpoint, arguments.interval, &redactor))
}

/// Envia cada linha válida e devolve um resumo com a contagem por resposta.
async fn replay(
    log: &str,
    endpoint: &GsiEndpoint,
    interval: Duration,
    redactor: &Redactor,
) -> Result<String, Box<Diagnostic>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
        .map_err(|error| {
            Diagnostic::unclassified(
                UiText::new("Reenvio do log GSI falhou"),
                "Não foi possível criar o cliente HTTP.",
                &error,
                redactor,
            )
        })?;
    let mut statuses: BTreeMap<String, u32> = BTreeMap::new();
    let mut skipped = 0;
    for line in log.lines() {
        let Some(mut payload) = serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|mut entry| entry.get_mut("payload").map(Value::take))
            .filter(Value::is_object)
        else {
            skipped += 1;
            continue;
        };
        payload["auth"] = serde_json::json!({ "token": endpoint.token });
        let outcome = match client.post(endpoint.uri()).json(&payload).send().await {
            Ok(response) => format!("HTTP {}", response.status().as_u16()),
            // Só a categoria do erro: a mensagem do reqwest inclui a URL, sem dados do payload.
            Err(request_error) if request_error.is_connect() => "sem conexão".to_owned(),
            Err(_) => "falha no envio".to_owned(),
        };
        *statuses.entry(outcome).or_default() += 1;
        tokio::time::sleep(interval).await;
    }
    let counts: Vec<String> = statuses
        .iter()
        .map(|(outcome, count)| format!("{outcome}: {count}"))
        .collect();
    Ok(format!(
        "Eventos reenviados — {}; linhas ignoradas: {skipped}.",
        counts.join(", ")
    ))
}
