//! Dedicated in-game adapter service for the guide server.
//!
//! The service owns a blocking loop that polls the loopback game gateway for
//! chat events and feeds each event through the shared `InGameChatBridge` and
//! the shared guide agent. The bridge mutex and the agent read lock are held
//! only for the current event and released before the next poll, so the Web
//! server can keep serving the same agent between events. The shutdown flag is
//! checked only between events: a running provider call is never aborted
//! mid-ask because the provider and gateway timeouts already own bounded
//! completion.

use guide_adapter::{GameAdapterRuntime, InGameChatBridge};
use guide_agent::GuideAgent;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

/// Default environment variable that carries the adapter bearer token.
pub const DEFAULT_TOKEN_ENVIRONMENT: &str = "PALWORLD_GUIDER_GATEWAY_TOKEN";

/// Fixed frame-size cap for the loopback adapter gateway.
pub const GATEWAY_MAX_PAYLOAD_BYTES: usize = 65_536;

/// Gateway tool-call timeout is never larger than this 3-second default.
pub const GATEWAY_CALL_TIMEOUT_CAP: Duration = Duration::from_secs(3);

const MIN_TOKEN_CHARACTERS: usize = 16;
const MAX_TOKEN_CHARACTERS: usize = 4096;

/// Parsed adapter-mode options used by the CLI to start the gateway service.
pub struct AdapterServiceOptions {
    pub port: u16,
    pub token_env: String,
    pub call_timeout: Duration,
    pub poll_interval: Duration,
}

/// Read the adapter bearer token from the named environment variable and
/// validate it. The token value is never printed, logged, or returned by any
/// other path; callers receive it only to bind the gateway.
pub fn adapter_environment_token(token_env: &str) -> Result<String, String> {
    validate_token_environment(token_env)?;
    let token = env::var(token_env).map_err(|_| {
        format!(
            "adapter token environment variable {token_env} is required; set it to a {MIN_TOKEN_CHARACTERS}..={MAX_TOKEN_CHARACTERS} character bearer token"
        )
    })?;
    validate_token(&token)
        .map_err(|error| format!("adapter token environment variable {token_env}: {error}"))?;
    Ok(token)
}

fn validate_token_environment(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("adapter token environment name must not be empty".to_string());
    }
    if name.chars().any(char::is_whitespace) {
        return Err("adapter token environment name must not contain whitespace".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err(
            "adapter token environment name must not contain control characters".to_string(),
        );
    }
    Ok(())
}

fn validate_token(token: &str) -> Result<(), String> {
    let characters = token.chars().count();
    if !(MIN_TOKEN_CHARACTERS..=MAX_TOKEN_CHARACTERS).contains(&characters) {
        return Err(format!(
            "token must be between {MIN_TOKEN_CHARACTERS} and {MAX_TOKEN_CHARACTERS} characters"
        ));
    }
    if token.trim() != token {
        return Err("token must not have surrounding whitespace".to_string());
    }
    if token.chars().any(char::is_control) {
        return Err("token must not contain control characters".to_string());
    }
    Ok(())
}

/// Spawn the dedicated adapter service thread. The returned handle resolves to
/// `Ok(())` on a clean shutdown and to an error string if the service fails.
pub fn start_adapter_service(
    agent: Arc<RwLock<GuideAgent>>,
    runtime: GameAdapterRuntime,
    bridge: Arc<Mutex<InGameChatBridge>>,
    shutdown: Arc<AtomicBool>,
) -> JoinHandle<Result<(), String>> {
    std::thread::Builder::new()
        .name("palworld-guider-adapter".into())
        .spawn(move || adapter_service_loop(agent, runtime, bridge, shutdown))
        .expect("adapter service thread starts")
}

fn adapter_service_loop(
    agent: Arc<RwLock<GuideAgent>>,
    runtime: GameAdapterRuntime,
    bridge: Arc<Mutex<InGameChatBridge>>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
    let poll_interval = bridge
        .lock()
        .map_err(|_| "adapter chat bridge lock poisoned".to_string())?
        .poll_interval();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }
        match runtime.next_event_timeout(poll_interval) {
            Ok(Some(event)) => {
                // Hold the bridge and the agent only for this one event;
                // provider work completes within the agent's own timeout.
                let mut bridge = bridge
                    .lock()
                    .map_err(|_| "adapter chat bridge lock poisoned".to_string())?;
                let agent = agent
                    .read()
                    .map_err(|_| "shared agent lock poisoned".to_string())?;
                bridge.process_event(&agent, event);
            }
            Ok(None) => {}
            Err(_) => {
                // A gateway failure must never take the guide down: keep
                // polling. Delivery failures inside process_event are already
                // captured by the bridge and never propagate here.
            }
        }
    }
}
