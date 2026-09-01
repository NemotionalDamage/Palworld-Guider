//! In-game adapter runtime and bounded chat bridge for the Palworld Guider.

pub mod chat;
pub mod runtime;

pub use chat::{ChatOutcome, InGameChatBridge, InGameLimits, CHAT_PREFIX};
pub use runtime::{
    AdapterError, GameAdapterRuntime, INTERNAL_RUNTIME_TOOLS, MODEL_VISIBLE_RUNTIME_TOOLS,
};
