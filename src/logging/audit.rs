use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub struct AuditLogger {
    path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    LlmRequest,
    LlmResponse,
    TradeDecision,
    RuleValidation,
    OrderSubmitted,
    OrderFilled,
    OrderRejected,
    RuleViolation,
    Error,
}

impl AuditLogger {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn log(&self, entry: AuditEntry) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        let line = serde_json::to_string(&entry).unwrap_or_default() + "\n";
        file.write_all(line.as_bytes()).await?;
        Ok(())
    }
}
