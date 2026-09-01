//! Bounded in-game chat bridge between the guide agent and the adapter.
//!
//! Deterministic behavior:
//! - Events whose text is empty after stripping [`CHAT_PREFIX`] are ignored
//!   with no provider run and no delivery. The returned `ChatOutcome` carries
//!   a synthesized `AgentAnswer` with `AgentStatus::Error` and an explanatory
//!   error, `delivered: false`, and no delivery error.
//! - The exact question `ping` (case-insensitive, after trim) bypasses the
//!   provider entirely and sends one reply with the exact text
//!   `Pong: Palworld Guider adapter connected.`. Ping does not consume the
//!   per-minute ask budget and is not recorded in guide history.
//! - Non-ping questions are clamped to `max_question_characters`, run through
//!   `GuideAgent::ask_with_cancellation` exactly once, and the final reply is
//!   sent exactly once through `send_chat` (no retry loop).
//! - History keeps the newest `max_history_exchanges` processed exchanges as
//!   text-only `Q:`/`A:` pairs; no JSON, tokens, or paths enter the prompts.
//!   The exchange is recorded when the agent produces a reply, before
//!   delivery, so a failed delivery does not lose the follow-up context.
//! - A rolling `max_asks_per_minute` window fails closed: an event that would
//!   exceed the budget is ignored with no provider run and no delivery
//!   (`AgentStatus::Error`, error `chat rate limit exceeded; try again later`).
//! - Replies are clamped to `max_reply_characters` before delivery. When the
//!   agent produced no usable answer (provider or agent failure), the reply
//!   starts with `Guide unavailable:` and never fabricates content. The
//!   original `AgentAnswer` is preserved unchanged in the `ChatOutcome`.

use crate::runtime::GameAdapterRuntime;
use game_gateway::ChatEvent;
use guide_agent::{AgentAnswer, AgentStatus, GuideAgent};
use guide_core::VersionInfo;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// Prefix a player must use to address the guide in in-game chat.
pub const CHAT_PREFIX: &str = "!guide ";

const PONG_REPLY: &str = "Pong: Palworld Guider adapter connected.";
const GUIDE_UNAVAILABLE_PREFIX: &str = "Guide unavailable:";
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct InGameLimits {
    pub max_history_exchanges: usize,
    pub max_asks_per_minute: usize,
    pub max_question_characters: usize,
    pub max_reply_characters: usize,
    pub poll_interval: Duration,
}

impl Default for InGameLimits {
    fn default() -> Self {
        Self {
            max_history_exchanges: 4,
            max_asks_per_minute: 6,
            max_question_characters: 1000,
            max_reply_characters: 400,
            poll_interval: Duration::from_millis(75),
        }
    }
}

/// Bounded adapter bridge: bounded history, rolling ask budget, and one
/// provider run plus one delivery per processed event.
pub struct InGameChatBridge {
    runtime: GameAdapterRuntime,
    limits: InGameLimits,
    history: VecDeque<(String, String)>,
    ask_timestamps: VecDeque<Instant>,
}

pub struct ChatOutcome {
    pub event_id: String,
    pub answer: AgentAnswer,
    pub delivered: bool,
    pub delivery_error: Option<String>,
}

impl InGameChatBridge {
    pub fn new(runtime: GameAdapterRuntime, limits: InGameLimits) -> Self {
        Self {
            runtime,
            limits,
            history: VecDeque::new(),
            ask_timestamps: VecDeque::new(),
        }
    }

    /// The configured poll interval used by the adapter service loop.
    pub fn poll_interval(&self) -> Duration {
        self.limits.poll_interval
    }

    pub fn process_event(&mut self, agent: &GuideAgent, event: ChatEvent) -> ChatOutcome {
        let stripped = event.text.strip_prefix(CHAT_PREFIX).unwrap_or(&event.text);
        let question = stripped.trim();
        if question.is_empty() {
            return self.skip_event(
                event,
                "ignored chat event: no question after the guide prefix",
            );
        }
        if question.eq_ignore_ascii_case("ping") {
            return self.pong(event);
        }
        if self.rate_limited() {
            return self.skip_event(event, "chat rate limit exceeded; try again later");
        }

        let question = clamp(question, self.limits.max_question_characters);
        self.record_ask();
        let prompt = self.build_prompt(&question);
        let answer = agent.ask_with_cancellation(&prompt, &AtomicBool::new(false));
        let reply = clamp(&self.reply_for(&answer), self.limits.max_reply_characters);
        self.record_exchange(question, reply.clone());
        self.deliver(event, answer, reply)
    }

    fn build_prompt(&self, question: &str) -> String {
        let mut lines = Vec::with_capacity(self.history.len() + 1);
        for (past_question, past_answer) in &self.history {
            lines.push(format!("Q: {past_question}\nA: {past_answer}"));
        }
        lines.push(format!("Q: {question}"));
        lines.join("\n")
    }

    fn reply_for(&self, answer: &AgentAnswer) -> String {
        match (&answer.answer, answer.status) {
            (Some(_), AgentStatus::Error) => unavailable_reply(answer),
            (Some(text), _) => text.clone(),
            (None, _) => unavailable_reply(answer),
        }
    }

    fn deliver(&self, event: ChatEvent, answer: AgentAnswer, reply: String) -> ChatOutcome {
        match self.runtime.send_chat(&reply) {
            Ok(()) => ChatOutcome {
                event_id: event.event_id,
                answer,
                delivered: true,
                delivery_error: None,
            },
            Err(error) => ChatOutcome {
                event_id: event.event_id,
                answer,
                delivered: false,
                delivery_error: Some(error.to_string()),
            },
        }
    }

    fn pong(&self, event: ChatEvent) -> ChatOutcome {
        let reply = clamp(PONG_REPLY, self.limits.max_reply_characters);
        let answer = AgentAnswer {
            status: AgentStatus::Ok,
            answer: Some(reply.clone()),
            tool_calls: Vec::new(),
            provenance: Vec::new(),
            version: unversioned_info(),
            uncertainty: Vec::new(),
            errors: Vec::new(),
        };
        self.deliver(event, answer, reply)
    }

    fn skip_event(&self, event: ChatEvent, reason: &str) -> ChatOutcome {
        ChatOutcome {
            event_id: event.event_id,
            answer: AgentAnswer {
                status: AgentStatus::Error,
                answer: None,
                tool_calls: Vec::new(),
                provenance: Vec::new(),
                version: unversioned_info(),
                uncertainty: Vec::new(),
                errors: vec![reason.to_string()],
            },
            delivered: false,
            delivery_error: None,
        }
    }

    fn record_ask(&mut self) {
        self.ask_timestamps.push_back(Instant::now());
    }

    fn rate_limited(&mut self) -> bool {
        let now = Instant::now();
        while self
            .ask_timestamps
            .front()
            .is_some_and(|first| now.duration_since(*first) >= RATE_LIMIT_WINDOW)
        {
            self.ask_timestamps.pop_front();
        }
        self.ask_timestamps.len() >= self.limits.max_asks_per_minute
    }

    fn record_exchange(&mut self, question: String, answer: String) {
        self.history.push_back((question, answer));
        while self.history.len() > self.limits.max_history_exchanges {
            self.history.pop_front();
        }
    }
}

fn unavailable_reply(answer: &AgentAnswer) -> String {
    let detail = answer
        .errors
        .first()
        .map(String::as_str)
        .unwrap_or("no answer was produced");
    format!("{GUIDE_UNAVAILABLE_PREFIX} {detail}")
}

fn clamp(text: &str, max_characters: usize) -> String {
    text.chars().take(max_characters).collect()
}

fn unversioned_info() -> VersionInfo {
    VersionInfo {
        knowledge_version: "unavailable".to_string(),
        configured_game_version: None,
        matches: false,
    }
}
