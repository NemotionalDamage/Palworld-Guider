use guide_agent::AgentAnswer;
use state_snapshot::{PlayerStateSnapshot, SnapshotFreshness};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct ServerLimits {
    pub session_ttl: Duration,
    pub ask_timeout: Duration,
    pub max_sessions: usize,
    pub max_history_exchanges: usize,
    pub max_asks_per_minute: usize,
    pub max_snapshots_per_minute: usize,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            session_ttl: Duration::from_secs(1800),
            ask_timeout: Duration::from_secs(30),
            max_sessions: 32,
            max_history_exchanges: 4,
            max_asks_per_minute: 10,
            max_snapshots_per_minute: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    NotFound,
    Expired,
    SessionLimitReached,
    AskRateLimited { retry_after_secs: u64 },
    SnapshotRateLimited { retry_after_secs: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotMetadata {
    pub schema_version: String,
    pub source_kind: String,
    pub game_version: String,
    pub freshness: SnapshotFreshness,
    pub missing_fields: Vec<String>,
}

impl SnapshotMetadata {
    fn from_snapshot(snapshot: &PlayerStateSnapshot) -> Self {
        Self {
            schema_version: snapshot.schema_version.clone(),
            source_kind: snapshot.source.kind.clone(),
            game_version: snapshot.source.game_version.clone(),
            freshness: snapshot.freshness(chrono::Utc::now()),
            missing_fields: snapshot.completeness().missing_fields,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ExchangeRecord {
    pub question: String,
    pub answer: AgentAnswer,
}

impl ExchangeRecord {
    pub fn new(question: impl Into<String>, answer: AgentAnswer) -> Self {
        Self {
            question: question.into(),
            answer,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: String,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub exchanges: Vec<ExchangeRecord>,
    pub snapshot: Option<SnapshotMetadata>,
    ask_times: Vec<Instant>,
    snapshot_times: Vec<Instant>,
    snapshot_value: Option<PlayerStateSnapshot>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    limits: ServerLimits,
    sessions: HashMap<String, SessionRecord>,
}

impl SessionStore {
    pub fn new(limits: ServerLimits) -> Self {
        Self {
            limits,
            sessions: HashMap::new(),
        }
    }

    pub fn create_session(&mut self, now: Instant) -> Result<SessionRecord, SessionError> {
        if self.sessions.len() >= self.limits.max_sessions {
            return Err(SessionError::SessionLimitReached);
        }
        let record = SessionRecord {
            id: format!("{:032x}", rand::random::<u128>()),
            created_at: now,
            expires_at: now + self.limits.session_ttl,
            exchanges: Vec::new(),
            snapshot: None,
            ask_times: Vec::new(),
            snapshot_times: Vec::new(),
            snapshot_value: None,
        };
        self.sessions.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    pub fn session(&self, session_id: &str, now: Instant) -> Result<SessionRecord, SessionError> {
        let record = self
            .sessions
            .get(session_id)
            .ok_or(SessionError::NotFound)?;
        if now > record.expires_at {
            return Err(SessionError::Expired);
        }
        Ok(record.clone())
    }

    pub fn remove_expired(&mut self, now: Instant) -> Vec<String> {
        let expired = self
            .sessions
            .iter()
            .filter(|(_, record)| now > record.expires_at)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        self.sessions.retain(|_, record| now <= record.expires_at);
        expired
    }

    pub fn reserve_ask(
        &mut self,
        session_id: &str,
        question: &str,
        now: Instant,
    ) -> Result<String, SessionError> {
        let limits = self.limits;
        let record = self.live_session_mut(session_id, now)?;
        prune_window(&mut record.ask_times, now, Duration::from_secs(60));
        if record.ask_times.len() >= limits.max_asks_per_minute {
            return Err(SessionError::AskRateLimited {
                retry_after_secs: retry_after(record.ask_times[0], now),
            });
        }
        let question = question.trim();
        let mut prompt = String::from("Bounded guide context:\n");
        for exchange in record
            .exchanges
            .iter()
            .rev()
            .take(limits.max_history_exchanges)
            .rev()
        {
            let answer = exchange.answer.answer.as_deref().unwrap_or("[no answer]");
            prompt.push_str("Question: ");
            prompt.push_str(&exchange.question);
            prompt.push_str("\nAnswer: ");
            prompt.push_str(answer);
            prompt.push('\n');
        }
        prompt.push_str("Current question: ");
        prompt.push_str(question);
        record.ask_times.push(now);
        Ok(prompt)
    }

    pub fn complete_ask(
        &mut self,
        session_id: &str,
        question: &str,
        answer: AgentAnswer,
    ) -> Result<(), SessionError> {
        let now = Instant::now();
        let max_history = self.limits.max_history_exchanges;
        let record = self.live_session_mut(session_id, now)?;
        record.exchanges.push(ExchangeRecord::new(question, answer));
        if record.exchanges.len() > max_history {
            let overflow = record.exchanges.len() - max_history;
            record.exchanges.drain(0..overflow);
        }
        Ok(())
    }

    pub fn attach_snapshot(
        &mut self,
        session_id: &str,
        snapshot: PlayerStateSnapshot,
        now: Instant,
    ) -> Result<SnapshotMetadata, SessionError> {
        let limits = self.limits;
        let record = self.live_session_mut(session_id, now)?;
        prune_window(&mut record.snapshot_times, now, Duration::from_secs(60));
        if record.snapshot_times.len() >= limits.max_snapshots_per_minute {
            return Err(SessionError::SnapshotRateLimited {
                retry_after_secs: retry_after(record.snapshot_times[0], now),
            });
        }
        let metadata = SnapshotMetadata::from_snapshot(&snapshot);
        record.snapshot = Some(metadata.clone());
        record.snapshot_value = Some(snapshot);
        record.snapshot_times.push(now);
        Ok(metadata)
    }

    pub fn attached_snapshot(
        &self,
        session_id: &str,
        now: Instant,
    ) -> Result<PlayerStateSnapshot, SessionError> {
        self.session(session_id, now)?
            .snapshot_value
            .ok_or(SessionError::NotFound)
    }

    fn live_session_mut(
        &mut self,
        session_id: &str,
        now: Instant,
    ) -> Result<&mut SessionRecord, SessionError> {
        let expires_at = self
            .sessions
            .get(session_id)
            .ok_or(SessionError::NotFound)?
            .expires_at;
        if now > expires_at {
            return Err(SessionError::Expired);
        }
        Ok(self.sessions.get_mut(session_id).expect("session exists"))
    }
}

fn prune_window(times: &mut Vec<Instant>, now: Instant, window: Duration) {
    times.retain(|time| now.duration_since(*time) < window);
}

fn retry_after(oldest: Instant, now: Instant) -> u64 {
    let elapsed = now.duration_since(oldest).as_secs();
    60_u64.saturating_sub(elapsed).max(1)
}
