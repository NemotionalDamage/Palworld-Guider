//! Local Web interface for the Palworld guide.

mod session;

pub use session::{
    ExchangeRecord, ServerLimits, SessionError, SessionRecord, SessionStore, SnapshotMetadata,
};
