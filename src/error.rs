use thiserror::Error;

#[derive(Debug, Error)]
pub enum BotError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("OAuth error: {0}")]
    OAuth(String),

    #[error("Schwab API error: {status} - {message}")]
    SchwabApi { status: u16, message: String },

    #[error("Anthropic API error: {status} - {message}")]
    AnthropicApi { status: u16, message: String },

    #[error("Token expired, re-authentication required")]
    TokenExpired,

    #[error("Rule violation: {0}")]
    RuleViolation(#[from] crate::trading::rules::RuleViolation),

    #[error("Parse error: {0}")]
    Parse(#[from] crate::llm::response::ParseError),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BotError>;
