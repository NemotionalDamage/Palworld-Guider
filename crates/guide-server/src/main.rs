use guide_agent::{AgentConfig, AgentLimits, GuideAgent};
use guide_core::GuideEngine;
use guide_server::{GuideServer, ServerLimits};
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, OllamaProvider, OpenAiCompatibleProvider};
use serde_json::json;
use std::{env, path::PathBuf, process::ExitCode, time::Duration};

const USAGE: &str = "usage: guide-server --data <reviewed-data-directory> [--game-version <version>] [--port <1-65535>] [--timeout-seconds <seconds>]

The server is loopback-only and always binds 127.0.0.1.
Provider configuration comes only from GUIDE_PROVIDER, GUIDE_MODEL, GUIDE_BASE_URL, and OPENAI_API_KEY.";

struct Options {
    data: PathBuf,
    game_version: Option<String>,
    port: u16,
    timeout_seconds: u64,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let options = match parse_options(&arguments) {
        Ok(options) => options,
        Err(error) => {
            println!("{}", error_value(&error));
            return ExitCode::FAILURE;
        }
    };
    match run(options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!("{}", error_value(&error));
            ExitCode::FAILURE
        }
    }
}

fn parse_options(arguments: &[String]) -> Result<Options, String> {
    let mut data = None;
    let mut game_version = None;
    let mut port = 8070_u16;
    let mut timeout_seconds = 30_u64;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        match argument {
            "--data" => {
                index += 1;
                data = Some(PathBuf::from(next_argument(arguments, index, argument)?));
            }
            "--game-version" => {
                index += 1;
                game_version = Some(next_argument(arguments, index, argument)?);
            }
            "--port" => {
                index += 1;
                let value = next_argument(arguments, index, argument)?;
                port = value
                    .parse()
                    .map_err(|_| format!("invalid port: {value}"))?;
            }
            "--timeout-seconds" => {
                index += 1;
                let value = next_argument(arguments, index, argument)?;
                timeout_seconds = value
                    .parse()
                    .map_err(|_| format!("invalid timeout: {value}"))?;
                if timeout_seconds == 0 || timeout_seconds > 300 {
                    return Err("timeout must be between 1 and 300 seconds".to_string());
                }
            }
            "--host" => {
                return Err(
                    "unsupported --host option: guide-server is loopback-only and binds 127.0.0.1"
                        .to_string(),
                );
            }
            other => return Err(format!("unknown argument {other}; {USAGE}")),
        }
        index += 1;
    }
    Ok(Options {
        data: data.ok_or_else(|| format!("--data is required; {USAGE}"))?,
        game_version,
        port,
        timeout_seconds,
    })
}

fn next_argument(arguments: &[String], index: usize, name: &str) -> Result<String, String> {
    arguments
        .get(index)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value; {USAGE}"))
}

async fn run(options: Options) -> Result<(), String> {
    let store =
        game_knowledge::KnowledgeStore::load_directory(&options.data).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
    let index = KnowledgeIndex::from_store(&store, options.game_version.clone())?;
    let engine = GuideEngine::new(store, options.game_version);
    let timeout = Duration::from_secs(options.timeout_seconds);
    let provider_timeout = timeout;
    let provider =
        tokio::task::spawn_blocking(move || build_provider_from_environment(provider_timeout))
            .await
            .map_err(|error| format!("provider worker failed: {error}"))??;
    let agent = GuideAgent::new(
        ToolRegistry::new(engine, index),
        provider,
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 8,
                timeout,
            },
            max_reply_characters: 1200,
        },
    );
    let app = GuideServer::new(
        agent,
        ServerLimits {
            ask_timeout: timeout,
            ..ServerLimits::default()
        },
    )
    .router();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", options.port))
        .await
        .map_err(|error| format!("failed to bind 127.0.0.1:{}: {error}", options.port))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("failed to resolve local address: {error}"))?;
    println!(
        "Palworld Guider listening on http://127.0.0.1:{}",
        address.port()
    );
    axum::serve(listener, app)
        .await
        .map_err(|error| format!("server failed: {error}"))
}

fn build_provider_from_environment(timeout: Duration) -> Result<Box<dyn ChatProvider>, String> {
    let provider = env::var("GUIDE_PROVIDER")
        .map_err(|_| "GUIDE_PROVIDER environment variable is required".to_string())?;
    let model = env::var("GUIDE_MODEL")
        .map_err(|_| "GUIDE_MODEL environment variable is required".to_string())?;
    let base_url = env::var("GUIDE_BASE_URL").ok();
    match provider.as_str() {
        "openai" => {
            let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
                "OPENAI_API_KEY environment variable is required for the openai provider"
                    .to_string()
            })?;
            let endpoint = base_url
                .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string());
            Ok(Box::new(
                OpenAiCompatibleProvider::new(endpoint, api_key, model, timeout)
                    .map_err(|error| error.to_string())?,
            ))
        }
        "ollama" => {
            let base_url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
            Ok(Box::new(
                OllamaProvider::new(base_url, model, timeout).map_err(|error| error.to_string())?,
            ))
        }
        other => Err(format!(
            "unsupported GUIDE_PROVIDER {other}; expected openai or ollama"
        )),
    }
}

fn error_value(message: &str) -> serde_json::Value {
    json!({"status": "error", "message": message})
}
