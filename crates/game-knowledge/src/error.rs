use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub record_id: Option<String>,
    pub field: &'static str,
    pub message: String,
}

impl ValidationError {
    pub fn new(record_id: Option<String>, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            record_id,
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.record_id {
            Some(record_id) => write!(
                formatter,
                "record {}: {}: {}",
                record_id, self.field, self.message
            ),
            None => write!(formatter, "{}: {}", self.field, self.message),
        }
    }
}

impl std::error::Error for ValidationError {}
