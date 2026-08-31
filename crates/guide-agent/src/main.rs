use game_knowledge::KnowledgeStore;
use guide_agent::{AgentConfig, AgentLimits, AgentStatus, GuideAgent};
use guide_core::GuideEngine;
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{OllamaProvider, OpenAiCompatibleProvider};
use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage: guide-agent ask QUESTION --provider openai|ollama --model MODEL [--data DIR] [--base-url URL] [--game-version VERSION] [--max-tool-calls N] [--timeout-seconds N]";

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match run(arguments) {
        Ok((value, status)) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
            );
            if matches!(status, AgentStatus::Error) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(message) => {
            println!("{}", error_value(message));
            ExitCode::FAILURE
        }
    }
}

struct Options {
    question: String,
    data: PathBuf,
    provider: String,
    model: String,
    base_url: Option<String>,
    game_version: Option<String>,
    max_tool_calls: usize,
    timeout_seconds: u64,
}

fn parse_options(arguments: Vec<String>) -> Result<Options, String> {
    let mut arguments = arguments.into_iter();
    let command = arguments.next().ok_or_else(|| USAGE.to_string())?;
    if command != "ask" {
        return Err(USAGE.to_string());
    }
    let question = arguments.next().ok_or_else(|| USAGE.to_string())?;
    let mut data = PathBuf::from("data/reviewed");
    let mut provider: Option<String> = None;
    let mut model: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut game_version: Option<String> = None;
    let mut max_tool_calls = 6;
    let mut timeout_seconds = 60;
    let mut values = arguments.collect::<Vec<_>>();
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--data" => {
                index += 1;
                let value = values.get(index).ok_or("--data requires a path")?;
                data = PathBuf::from(value.clone());
            }
            "--provider" => {
                index += 1;
                let value = values
                    .get(index)
                    .ok_or("--provider requires openai or ollama")?;
                provider = Some(value.clone());
            }
            "--model" => {
                index += 1;
                let value = values.get(index).ok_or("--model requires a model name")?;
                model = Some(value.clone());
            }
            "--base-url" => {
                index += 1;
                let value = values.get(index).ok_or("--base-url requires a URL")?;
                base_url = Some(value.clone());
            }
            "--game-version" => {
                index += 1;
                let value = values.get(index).ok_or("--game-version requires a value")?;
                game_version = Some(value.clone());
            }
            "--max-tool-calls" => {
                index += 1;
                let value = values
                    .get(index)
                    .ok_or("--max-tool-calls requires a number")?;
                max_tool_calls = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid maximum tool calls: {value}"))?;
            }
            "--timeout-seconds" => {
                index += 1;
                let value = values
                    .get(index)
                    .ok_or("--timeout-seconds requires a number")?;
                timeout_seconds = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid timeout seconds: {value}"))?;
            }
            _ => return Err(USAGE.to_string()),
        }
        index += 1;
    }
    values.clear();
    let provider = provider.ok_or(USAGE.to_string())?;
    let model = model.ok_or(USAGE.to_string())?;
    Ok(Options {
        question,
        data,
        provider,
        model,
        base_url,
        game_version,
        max_tool_calls,
        timeout_seconds,
    })
}

fn run(arguments: Vec<String>) -> Result<(Value, AgentStatus), String> {
    let options = parse_options(arguments)?;
    let store = KnowledgeStore::load_directory(&options.data).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let index = KnowledgeIndex::from_store(&store, options.game_version.clone())?;
    let engine = GuideEngine::new(store, options.game_version);
    let registry = ToolRegistry::new(engine, index);
    let timeout = Duration::from_secs(options.timeout_seconds);
    let provider: Box<dyn provider::ChatProvider> = match options.provider.as_str() {
        "openai" => {
            let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
                "OPENAI_API_KEY environment variable is required for the openai provider"
                    .to_string()
            })?;
            let endpoint = options
                .base_url
                .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string());
            Box::new(
                OpenAiCompatibleProvider::new(endpoint, api_key, options.model, timeout)
                    .map_err(|error| error.to_string())?,
            )
        }
        "ollama" => {
            let base_url = options
                .base_url
                .or_else(|| env::var("OLLAMA_BASE_URL").ok())
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Box::new(
                OllamaProvider::new(base_url, options.model, timeout)
                    .map_err(|error| error.to_string())?,
            )
        }
        other => {
            return Err(format!(
                "unsupported provider: {other}; expected openai or ollama"
            ))
        }
    };
    let agent = GuideAgent::new(
        registry,
        provider,
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: options.max_tool_calls,
                timeout,
            },
            max_reply_characters: 1200,
        },
    );
    let answer = agent.ask(&options.question);
    let status = answer.status;
    Ok((
        serde_json::to_value(answer).map_err(|error| error.to_string())?,
        status,
    ))
}

fn error_value(message: impl Into<String>) -> Value {
    json!({
        "status": "error",
        "answer": null,
        "tool_calls": [],
        "provenance": [],
        "version": {
            "knowledge_version": "unknown",
            "configured_game_version": null,
            "matches": false
        },
        "uncertainty": [],
        "errors": [message.into()]
    })
}
