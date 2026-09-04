//! Knowledge maintenance and hardening tools.
//!
//! Provides version compatibility checks, knowledge auditing, and batch validation
//! for keeping the guide trustworthy across game updates and long sessions.

pub mod audit;
pub mod version;

pub use audit::{BatchValidation, ConflictSummary, KnowledgeAudit, SourceSummary, StaleRecord};
pub use version::{version_check, VersionDimension, VersionReport};
