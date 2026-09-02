use game_gateway::{WebSocketGateway, WebSocketGatewayConfig};
use guide_adapter::{GameAdapterRuntime, InGameChatBridge, InGameLimits};
use guide_agent::{AgentConfig, AgentLimits, GuideAgent};
use guide_core::GuideEngine;
use guide_server::{
    adapter_environment_token, start_adapter_service, AdapterServiceOptions, GuideServer,
    ServerLimits, DEFAULT_TOKEN_ENVIRONMENT, GATEWAY_CALL_TIMEOUT_CAP, GATEWAY_MAX_PAYLOAD_BYTES,
};
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{chat_completions_endpoint, ChatProvider, OllamaProvider, OpenAiCompatibleProvider};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::{env, path::PathBuf, process::ExitCode, time::Duration};

const USAGE: &str = "usage: guide-server --data <reviewed-data-directory> [--game-version <version>] [--port <1-65535>] [--timeout-seconds <seconds>] [--adapter-port <1-65535>] [--adapter-token-env <name>]

The server is loopback-only and always binds 127.0.0.1.
Provider configuration comes only from GUIDE_PROVIDER, GUIDE_MODEL, GUIDE_BASE_URL, and OPENAI_API_KEY.
In-game adapter mode requires --adapter-port; the bearer token is read only from the --adapter-token-env environment variable (default PALWORLD_GUIDER_GATEWAY_TOKEN) and is never printed.";

struct Options {
    data: PathBuf,
    game_version: Option<String>,
    port: u16,
    timeout_seconds: u64,
    adapter_port: Option<u16>,
    adapter_token_env: String,
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
    let mut adapter_port = None;
    let mut adapter_token_env = DEFAULT_TOKEN_ENVIRONMENT.to_string();
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
            "--adapter-port" => {
                index += 1;
                let value = next_argument(arguments, index, argument)?;
                let parsed: u16 = value
                    .parse()
                    .map_err(|_| format!("invalid adapter port: {value}"))?;
                if parsed == 0 {
                    return Err("adapter port must be between 1 and 65535".to_string());
                }
                adapter_port = Some(parsed);
            }
            "--adapter-token-env" => {
                index += 1;
                adapter_token_env = next_argument(arguments, index, argument)?;
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
        adapter_port,
        adapter_token_env,
    })
}

fn next_argument(arguments: &[String], index: usize, name: &str) -> Result<String, String> {
    arguments
        .get(index)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value; {USAGE}"))
}

async fn run(options: Options) -> Result<(), String> {
    let ask_timeout = Duration::from_secs(options.timeout_seconds);
    let adapter_options = options.adapter_port.map(|port| AdapterServiceOptions {
        port,
        token_env: options.adapter_token_env.clone(),
        call_timeout: GATEWAY_CALL_TIMEOUT_CAP.min(ask_timeout),
        poll_interval: InGameLimits::default().poll_interval,
    });

    // Validate the adapter bearer token before any listener binds. The value
    // is used only to bind the loopback gateway and is never printed.
    let adapter_token = match &adapter_options {
        Some(adapter) => Some(adapter_environment_token(&adapter.token_env)?),
        None => None,
    };

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
    let provider_timeout = ask_timeout;
    let provider =
        tokio::task::spawn_blocking(move || build_provider_from_environment(provider_timeout))
            .await
            .map_err(|error| format!("provider worker failed: {error}"))??;

    let mut registry = ToolRegistry::new(engine, index);
    let (runtime, bridge) = match (&adapter_options, &adapter_token) {
        (Some(adapter), Some(token)) => {
            let gateway = WebSocketGateway::bind(
                &format!("127.0.0.1:{}", adapter.port),
                token,
                WebSocketGatewayConfig {
                    max_payload_bytes: GATEWAY_MAX_PAYLOAD_BYTES,
                    call_timeout: adapter.call_timeout,
                },
            )
            .map_err(|error| {
                format!(
                    "failed to bind adapter gateway 127.0.0.1:{}: {error}",
                    adapter.port
                )
            })?;
            let runtime = GameAdapterRuntime::new(gateway);
            registry = registry
                .try_with_runtime_tools(Arc::new(runtime.clone()))
                .map_err(|error| format!("failed to register adapter runtime tools: {error}"))?;
            let mut bridge = InGameChatBridge::new(
                runtime.clone(),
                InGameLimits {
                    poll_interval: adapter.poll_interval,
                    ..InGameLimits::default()
                },
            );
            if let Some(path) = chat_debug_log_from_environment() {
                println!("chat-debug-log={path}");
                bridge = bridge.with_debug_log(path);
            }
            (Some(runtime), Some(bridge))
        }
        _ => (None, None),
    };
    let bridge = bridge.map(|bridge| Arc::new(Mutex::new(bridge)));

    let agent = GuideAgent::new(
        registry,
        provider,
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 8,
                timeout: ask_timeout,
            },
            max_reply_characters: 1200,
        },
    );
    // One shared agent backs both the Web server and the in-game adapter
    // service so they use the same tool registry and provider budget.
    let shared_agent = Arc::new(RwLock::new(agent));
    let app = GuideServer::new(
        shared_agent.clone(),
        ServerLimits {
            ask_timeout,
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

    let shutdown = Arc::new(AtomicBool::new(false));
    let adapter_handle = match (runtime, bridge) {
        (Some(runtime), Some(bridge)) => Some(start_adapter_service(
            shared_agent.clone(),
            runtime,
            bridge,
            shutdown.clone(),
        )),
        (None, None) => None,
        _ => unreachable!("adapter runtime and bridge are created together"),
    };

    println!(
        "Palworld Guider listening on http://127.0.0.1:{}",
        address.port()
    );
    if let Some(adapter) = &adapter_options {
        println!("adapter=127.0.0.1:{}", adapter.port);
        println!("adapter-token-env={}", adapter.token_env);
    }

    let result = axum::serve(listener, app)
        .await
        .map_err(|error| format!("server failed: {error}"));
    shutdown.store(true, Ordering::SeqCst);
    if let Some(handle) = adapter_handle {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(format!("adapter service failed: {error}")),
            Err(_) => return Err("adapter service thread panicked".to_string()),
        }
    }
    result
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
            let endpoint = chat_completions_endpoint(&endpoint);
            let disable_reasoning = env::var("GUIDE_DISABLE_REASONING")
                .ok()
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            Ok(Box::new(
                OpenAiCompatibleProvider::new(endpoint, api_key, model, timeout, disable_reasoning)
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

fn chat_debug_log_from_environment() -> Option<String> {
    env::var("PALWORLD_GUIDER_CHAT_DEBUG_LOG")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn error_value(message: &str) -> serde_json::Value {
    json!({"status": "error", "message": message})
}
