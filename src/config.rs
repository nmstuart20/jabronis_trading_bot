use rust_decimal::Decimal;
use secrecy::SecretString;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub schwab: SchwabConfig,
    pub anthropic: AnthropicConfig,
    pub trading: TradingConfig,
    pub data_sources: DataSourcesConfig,
    #[serde(default = "default_state_path")]
    pub state_path: PathBuf,
    #[serde(default)]
    pub logging: LoggingConfig,
}

fn default_state_path() -> PathBuf {
    PathBuf::from("data/state.json")
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_audit_path")]
    pub audit_path: PathBuf,
}

fn default_audit_path() -> PathBuf {
    PathBuf::from("logs/audit.jsonl")
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            audit_path: default_audit_path(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SchwabConfig {
    pub app_key: SecretString,
    pub app_secret: SecretString,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: SecretString,
    pub model: String,
    pub max_tokens: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TradingConfig {
    pub max_position_size_dollars: Decimal,
    pub max_position_pct: Decimal,
    pub max_daily_trades: u32,
    pub max_daily_loss_dollars: Decimal,
    pub day_trade_limit: u32,
    pub allowed_tickers: Option<Vec<String>>,
    #[serde(default)]
    pub blocked_tickers: Vec<String>,
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    #[serde(default = "default_watchlist")]
    pub watchlist: Vec<String>,
}

fn default_dry_run() -> bool {
    true
}

fn default_watchlist() -> Vec<String> {
    vec!["AAPL", "GOOGL", "MSFT", "AMZN", "NVDA", "SPY", "QQQ"]
        .into_iter()
        .map(String::from)
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct DataSourcesConfig {
    pub news_api_key: Option<SecretString>,
    pub sentiment_api_key: Option<SecretString>,
    pub quote_interval_secs: u64,
}

impl Settings {
    pub fn load() -> crate::error::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/settings").required(false))
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
