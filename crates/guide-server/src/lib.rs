//! Local Web interface and in-game adapter service for the Palworld guide.

mod adapter_service;
mod api;
mod error;
mod session;
mod ui;

pub use adapter_service::{
    adapter_environment_token, start_adapter_service, AdapterServiceOptions,
    DEFAULT_TOKEN_ENVIRONMENT, GATEWAY_CALL_TIMEOUT_CAP, GATEWAY_MAX_PAYLOAD_BYTES,
};
pub use api::{GuideServer, GuideServerState};
pub use error::{ApiErrorCode, ApiErrorResponse};

pub use session::{
    ExchangeRecord, ServerLimits, SessionError, SessionRecord, SessionStore, SnapshotMetadata,
};
