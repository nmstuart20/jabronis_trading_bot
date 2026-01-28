# LLM-Powered Schwab Trading Bot - Project Specification

## Overview

Build a Rust-based automated trading bot that uses an LLM (Claude API) to make trading decisions based on real-time market data, news, sentiment, and historical data. The bot interfaces with Schwab's official API for execution.

## Core Architecture Principles

1. **Defense in depth**: The LLM is advisory only. All trading rules and limits are enforced in Rust code, never delegated to the LLM.
2. **Prompt injection resistance**: Text data (news, sentiment) is sanitized and structured before reaching the LLM decision layer.
3. **Fail-safe defaults**: If anything is ambiguous or malformed, the bot does nothing rather than guessing.
4. **Full auditability**: Every decision, API call, and LLM interaction is logged.

## Project Structure

```
schwab-trading-bot/
├── Cargo.toml
├── .env.example                 # Template for secrets
├── config/
│   └── settings.toml            # Non-secret configuration
├── src/
│   ├── main.rs                  # Entry point, scheduler
│   ├── lib.rs                   # Module exports
│   ├── config.rs                # Configuration loading
│   ├── error.rs                 # Custom error types
│   ├── schwab/
│   │   ├── mod.rs
│   │   ├── auth.rs              # OAuth2 flow, token refresh
│   │   ├── client.rs            # API client wrapper
│   │   ├── models.rs            # Schwab API types
│   │   └── orders.rs            # Order creation/submission
│   ├── data/
│   │   ├── mod.rs
│   │   ├── quotes.rs            # Real-time quote fetching
│   │   ├── historical.rs        # Historical price data
│   │   ├── news.rs              # News ingestion
│   │   └── sentiment.rs         # Sentiment data/scoring
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── client.rs            # Anthropic API client
│   │   ├── prompts.rs           # Prompt templates
│   │   ├── sanitizer.rs         # Input sanitization
│   │   └── response.rs          # Response parsing/validation
│   ├── trading/
│   │   ├── mod.rs
│   │   ├── decision.rs          # Decision struct and validation
│   │   ├── rules.rs             # Day trading rules, limits
│   │   ├── portfolio.rs         # Position tracking
│   │   └── executor.rs          # Order execution logic
│   └── logging/
│       ├── mod.rs
│       └── audit.rs             # Trade audit logging
└── tests/
    ├── integration/
    └── mocks/
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "schwab-trading-bot"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "cookies"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
thiserror = "2"
anyhow = "1"
config = "0.14"
dotenvy = "0.15"
secrecy = "0.10"
url = "2"
rust_decimal = { version = "1", features = ["serde"] }
tokio-cron-scheduler = "0.13"

# For OAuth2
oauth2 = "4"
open = "5"               # Opens browser for auth flow
axum = "0.7"             # Local callback server for OAuth

# For LLM
anthropic-sdk = "0.1"    # Or use reqwest directly

[dev-dependencies]
wiremock = "0.6"
tokio-test = "0.4"
```

## Module Specifications

### 1. Configuration (`src/config.rs`)

Load configuration from `settings.toml` and environment variables.

```rust
// Settings structure
pub struct Settings {
    pub schwab: SchwabConfig,
    pub anthropic: AnthropicConfig,
    pub trading: TradingConfig,
    pub data_sources: DataSourcesConfig,
}

pub struct SchwabConfig {
    pub client_id: SecretString,
    pub client_secret: SecretString,
    pub redirect_uri: String,      // Usually http://localhost:8080/callback
    pub account_id: String,
}

pub struct AnthropicConfig {
    pub api_key: SecretString,
    pub model: String,             // e.g., "claude-sonnet-4-20250514"
    pub max_tokens: u32,
}

pub struct TradingConfig {
    pub max_position_size_dollars: Decimal,
    pub max_position_pct: Decimal,           // Max % of portfolio in one position
    pub max_daily_trades: u32,
    pub max_daily_loss_dollars: Decimal,
    pub day_trade_limit: u32,                // PDT rule: 3 round trips per 5 days
    pub allowed_tickers: Option<Vec<String>>, // Whitelist if desired
    pub blocked_tickers: Vec<String>,        // Blacklist
}

pub struct DataSourcesConfig {
    pub news_api_key: Option<SecretString>,
    pub sentiment_api_key: Option<SecretString>,
    pub quote_interval_secs: u64,
}
```

### 2. Schwab Authentication (`src/schwab/auth.rs`)

Implement OAuth2 PKCE flow for Schwab API.

**Requirements:**
- First-time auth: Open browser, run local callback server, exchange code for tokens
- Store tokens securely (encrypted file or keyring)
- Auto-refresh tokens before expiry
- Handle refresh token expiration (re-auth needed)

```rust
pub struct TokenManager {
    // Internal state
}

impl TokenManager {
    pub async fn new(config: &SchwabConfig) -> Result<Self>;
    pub async fn get_access_token(&self) -> Result<SecretString>;
    pub async fn refresh_if_needed(&self) -> Result<()>;
    pub async fn initiate_auth_flow(&self) -> Result<()>; // Browser-based
}
```

**Schwab OAuth endpoints:**
- Authorization: `https://api.schwabapi.com/v1/oauth/authorize`
- Token: `https://api.schwabapi.com/v1/oauth/token`

### 3. Schwab Client (`src/schwab/client.rs`)

Wrapper for Schwab Trader API.

```rust
pub struct SchwabClient {
    http: reqwest::Client,
    token_manager: TokenManager,
    base_url: String,
}

impl SchwabClient {
    // Account info
    pub async fn get_accounts(&self) -> Result<Vec<Account>>;
    pub async fn get_account(&self, account_id: &str) -> Result<Account>;
    
    // Quotes
    pub async fn get_quote(&self, symbol: &str) -> Result<Quote>;
    pub async fn get_quotes(&self, symbols: &[&str]) -> Result<HashMap<String, Quote>>;
    
    // Price history
    pub async fn get_price_history(
        &self,
        symbol: &str,
        period_type: PeriodType,
        period: u32,
        frequency_type: FrequencyType,
        frequency: u32,
    ) -> Result<PriceHistory>;
    
    // Orders
    pub async fn place_order(&self, account_id: &str, order: &Order) -> Result<OrderResponse>;
    pub async fn get_orders(&self, account_id: &str) -> Result<Vec<Order>>;
    pub async fn cancel_order(&self, account_id: &str, order_id: &str) -> Result<()>;
    
    // Positions
    pub async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>>;
}
```

**Base URL:** `https://api.schwabapi.com/trader/v1`

### 4. Schwab Models (`src/schwab/models.rs`)

Define types matching Schwab API responses.

```rust
#[derive(Debug, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub total_volume: u64,
    pub high_price: Decimal,
    pub low_price: Decimal,
    pub open_price: Decimal,
    pub close_price: Decimal,
    pub quote_time: i64,
}

#[derive(Debug, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub current_value: Decimal,
    pub unrealized_pnl: Decimal,
}

#[derive(Debug, Serialize)]
pub struct Order {
    pub order_type: OrderType,
    pub session: Session,
    pub duration: Duration,
    pub order_strategy_type: OrderStrategyType,
    pub order_leg_collection: Vec<OrderLeg>,
}

#[derive(Debug, Serialize)]
pub struct OrderLeg {
    pub instruction: Instruction,  // BUY, SELL
    pub quantity: u32,
    pub instrument: Instrument,
}

#[derive(Debug, Serialize)]
pub struct Instrument {
    pub symbol: String,
    pub asset_type: AssetType,
}

// Enums for order types, sessions, etc.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Session {
    Normal,
    Am,
    Pm,
    Seamless,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Duration {
    Day,
    GoodTillCancel,
    FillOrKill,
}
```

### 5. Data Ingestion (`src/data/`)

#### quotes.rs
```rust
pub struct QuoteService {
    schwab: Arc<SchwabClient>,
    cache: DashMap<String, CachedQuote>,
}

impl QuoteService {
    pub async fn get_current_quotes(&self, symbols: &[&str]) -> Result<HashMap<String, Quote>>;
}
```

#### historical.rs
```rust
pub struct HistoricalDataService {
    schwab: Arc<SchwabClient>,
}

impl HistoricalDataService {
    pub async fn get_daily_bars(&self, symbol: &str, days: u32) -> Result<Vec<Bar>>;
    pub async fn get_intraday_bars(&self, symbol: &str, minutes: u32) -> Result<Vec<Bar>>;
}
```

#### news.rs
```rust
// Use a news API (e.g., NewsAPI, Finnhub, or Alpha Vantage)
pub struct NewsService {
    http: reqwest::Client,
    api_key: SecretString,
}

impl NewsService {
    pub async fn get_news(&self, symbols: &[&str], limit: usize) -> Result<Vec<NewsItem>>;
}

#[derive(Debug)]
pub struct NewsItem {
    pub headline: String,
    pub summary: String,
    pub source: String,
    pub published_at: DateTime<Utc>,
    pub symbols: Vec<String>,
}
```

#### sentiment.rs
```rust
pub struct SentimentService {
    http: reqwest::Client,
}

impl SentimentService {
    // Convert raw sentiment data to numerical scores
    pub async fn get_sentiment_scores(&self, symbols: &[&str]) -> Result<HashMap<String, SentimentScore>>;
}

#[derive(Debug)]
pub struct SentimentScore {
    pub symbol: String,
    pub score: f64,          // -1.0 to 1.0
    pub volume: u32,         // Number of mentions
    pub source: String,
}
```

### 6. LLM Integration (`src/llm/`)

#### sanitizer.rs

**Critical for prompt injection defense.**

```rust
pub struct InputSanitizer;

impl InputSanitizer {
    /// Sanitize news headlines and summaries
    pub fn sanitize_news(news: &[NewsItem]) -> Vec<SanitizedNews> {
        news.iter()
            .map(|item| SanitizedNews {
                headline: Self::sanitize_text(&item.headline, 200),
                summary: Self::sanitize_text(&item.summary, 500),
                source: Self::sanitize_text(&item.source, 50),
                published_at: item.published_at,
                symbols: item.symbols.clone(),
            })
            .collect()
    }
    
    /// Core text sanitization
    fn sanitize_text(text: &str, max_len: usize) -> String {
        text
            // Remove potential injection patterns
            .replace("ignore", "")
            .replace("IGNORE", "")
            .replace("disregard", "")
            .replace("DISREGARD", "")
            .replace("instruction", "")
            .replace("INSTRUCTION", "")
            .replace("system", "")
            .replace("SYSTEM", "")
            // Remove special characters that might be used for injection
            .replace('<', "")
            .replace('>', "")
            .replace('{', "")
            .replace('}', "")
            // Truncate
            .chars()
            .take(max_len)
            .collect()
    }
    
    /// Convert complex data to simple structured format
    pub fn structure_market_context(
        quotes: &HashMap<String, Quote>,
        historical: &HashMap<String, Vec<Bar>>,
        sentiment: &HashMap<String, SentimentScore>,
        news: &[SanitizedNews],
    ) -> MarketContext {
        // Build clean, structured context
    }
}

#[derive(Debug, Serialize)]
pub struct MarketContext {
    pub timestamp: DateTime<Utc>,
    pub quotes: Vec<QuoteSummary>,
    pub sentiment_scores: Vec<SentimentSummary>,
    pub news_summaries: Vec<NewsSummary>,
    pub technical_indicators: Vec<TechnicalSummary>,
}
```

#### prompts.rs

```rust
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_trading_prompt(
        context: &MarketContext,
        portfolio: &Portfolio,
        constraints: &TradingConstraints,
    ) -> String {
        format!(r#"
You are a trading assistant. Analyze the market data and decide on a trading action.

## Current Portfolio
Cash Available: ${cash}
Positions: {positions}

## Constraints (STRICTLY ENFORCED - you cannot override these)
- Max position size: ${max_pos} or {max_pct}% of portfolio
- Remaining day trades this week: {day_trades_left}
- Max daily loss remaining: ${loss_remaining}
- Allowed to trade: {allowed_tickers}

## Current Market Data
{market_context}

## Your Task
Analyze the data and recommend ONE action. You must respond with ONLY a JSON object in this exact format:

```json
{{
  "action": "BUY" | "SELL" | "HOLD",
  "ticker": "SYMBOL" | null,
  "quantity": number | null,
  "order_type": "MARKET" | "LIMIT",
  "limit_price": number | null,
  "reasoning": "Brief explanation (max 200 chars)"
}}
```

If no good opportunity exists, respond with action "HOLD".
Do not include any text outside the JSON object.
"#,
            cash = portfolio.cash_available,
            positions = format_positions(&portfolio.positions),
            max_pos = constraints.max_position_dollars,
            max_pct = constraints.max_position_pct,
            day_trades_left = constraints.day_trades_remaining,
            loss_remaining = constraints.daily_loss_remaining,
            allowed_tickers = constraints.allowed_tickers.join(", "),
            market_context = serde_json::to_string_pretty(&context).unwrap(),
        )
    }
}
```

#### response.rs

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LlmDecision {
    pub action: Action,
    pub ticker: Option<String>,
    pub quantity: Option<u32>,
    pub order_type: OrderType,
    pub limit_price: Option<Decimal>,
    pub reasoning: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

pub struct ResponseParser;

impl ResponseParser {
    pub fn parse(response: &str) -> Result<LlmDecision, ParseError> {
        // Extract JSON from response (handle markdown code blocks)
        let json_str = Self::extract_json(response)?;
        
        // Parse JSON
        let decision: LlmDecision = serde_json::from_str(&json_str)
            .map_err(|e| ParseError::InvalidJson(e.to_string()))?;
        
        // Validate fields
        Self::validate(&decision)?;
        
        Ok(decision)
    }
    
    fn extract_json(text: &str) -> Result<&str, ParseError> {
        // Handle ```json ... ``` blocks
        // Handle raw JSON
        // Return error if no valid JSON found
    }
    
    fn validate(decision: &LlmDecision) -> Result<(), ParseError> {
        // If action is BUY or SELL, ticker and quantity must be present
        // Quantity must be positive
        // Limit price required if order_type is LIMIT
        // Reasoning must not be empty
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("No JSON found in response")]
    NoJson,
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}
```

#### client.rs

```rust
pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: SecretString,
    model: String,
    max_tokens: u32,
}

impl AnthropicClient {
    pub async fn complete(&self, prompt: &str) -> Result<String> {
        let response = self.http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.model,
                "max_tokens": self.max_tokens,
                "messages": [
                    {"role": "user", "content": prompt}
                ]
            }))
            .send()
            .await?;
        
        // Parse response, extract text content
    }
}
```

### 7. Trading Logic (`src/trading/`)

#### rules.rs

**This is where all safety enforcement happens.**

```rust
pub struct TradingRules {
    config: TradingConfig,
    trade_history: Vec<TradeRecord>,
    daily_pnl: Decimal,
}

impl TradingRules {
    /// Check if a proposed trade is allowed
    pub fn validate_trade(
        &self,
        decision: &LlmDecision,
        portfolio: &Portfolio,
        quote: &Quote,
    ) -> Result<(), RuleViolation> {
        // Check 1: Is ticker allowed?
        self.check_ticker_allowed(&decision.ticker)?;
        
        // Check 2: Position size limits
        self.check_position_size(decision, portfolio, quote)?;
        
        // Check 3: Day trading rule (Pattern Day Trader)
        self.check_day_trade_limit(decision, portfolio)?;
        
        // Check 4: Daily loss limit
        self.check_daily_loss_limit()?;
        
        // Check 5: Daily trade count
        self.check_daily_trade_count()?;
        
        Ok(())
    }
    
    fn check_day_trade_limit(
        &self,
        decision: &LlmDecision,
        portfolio: &Portfolio,
    ) -> Result<(), RuleViolation> {
        // PDT Rule: Cannot make more than 3 day trades in 5 business days
        // if account < $25,000
        
        // A day trade = buying and selling same security same day
        // Check if this would create a day trade
        if decision.action == Action::Sell {
            if let Some(ticker) = &decision.ticker {
                // Check if we bought this today
                let bought_today = self.trade_history.iter().any(|t| {
                    t.ticker == *ticker 
                    && t.action == Action::Buy 
                    && t.timestamp.date() == Utc::now().date()
                });
                
                if bought_today {
                    let day_trades_this_week = self.count_day_trades_rolling_5_days();
                    if day_trades_this_week >= self.config.day_trade_limit {
                        return Err(RuleViolation::DayTradeLimitExceeded);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn check_position_size(
        &self,
        decision: &LlmDecision,
        portfolio: &Portfolio,
        quote: &Quote,
    ) -> Result<(), RuleViolation> {
        if decision.action != Action::Buy {
            return Ok(());
        }
        
        let quantity = decision.quantity.unwrap_or(0);
        let trade_value = Decimal::from(quantity) * quote.ask_price;
        
        // Check absolute limit
        if trade_value > self.config.max_position_size_dollars {
            return Err(RuleViolation::PositionTooLarge {
                requested: trade_value,
                max: self.config.max_position_size_dollars,
            });
        }
        
        // Check percentage limit
        let portfolio_value = portfolio.total_value();
        let pct = trade_value / portfolio_value * Decimal::from(100);
        if pct > self.config.max_position_pct {
            return Err(RuleViolation::PositionPercentageTooHigh {
                requested_pct: pct,
                max_pct: self.config.max_position_pct,
            });
        }
        
        // Check available cash
        if trade_value > portfolio.cash_available {
            return Err(RuleViolation::InsufficientCash {
                requested: trade_value,
                available: portfolio.cash_available,
            });
        }
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuleViolation {
    #[error("Ticker {0} is not allowed")]
    TickerNotAllowed(String),
    #[error("Position too large: ${requested}, max ${max}")]
    PositionTooLarge { requested: Decimal, max: Decimal },
    #[error("Position percentage too high: {requested_pct}%, max {max_pct}%")]
    PositionPercentageTooHigh { requested_pct: Decimal, max_pct: Decimal },
    #[error("Day trade limit exceeded (PDT rule)")]
    DayTradeLimitExceeded,
    #[error("Daily loss limit reached")]
    DailyLossLimitReached,
    #[error("Daily trade count exceeded")]
    DailyTradeCountExceeded,
    #[error("Insufficient cash: need ${requested}, have ${available}")]
    InsufficientCash { requested: Decimal, available: Decimal },
}
```

#### executor.rs

```rust
pub struct TradeExecutor {
    schwab: Arc<SchwabClient>,
    rules: TradingRules,
    account_id: String,
}

impl TradeExecutor {
    pub async fn execute(
        &mut self,
        decision: LlmDecision,
        portfolio: &Portfolio,
    ) -> Result<ExecutionResult> {
        // Step 1: If HOLD, do nothing
        if decision.action == Action::Hold {
            return Ok(ExecutionResult::Held { reason: decision.reasoning });
        }
        
        let ticker = decision.ticker.as_ref()
            .ok_or(ExecutorError::MissingTicker)?;
        
        // Step 2: Get fresh quote
        let quote = self.schwab.get_quote(ticker).await?;
        
        // Step 3: Validate against rules (THIS IS THE CRITICAL GATE)
        self.rules.validate_trade(&decision, portfolio, &quote)?;
        
        // Step 4: Build order
        let order = self.build_order(&decision, &quote)?;
        
        // Step 5: Submit order
        let response = self.schwab
            .place_order(&self.account_id, &order)
            .await?;
        
        // Step 6: Record trade
        self.rules.record_trade(TradeRecord {
            timestamp: Utc::now(),
            ticker: ticker.clone(),
            action: decision.action,
            quantity: decision.quantity.unwrap(),
            price: quote.last_price,
            order_id: response.order_id.clone(),
        });
        
        Ok(ExecutionResult::Executed {
            order_id: response.order_id,
            ticker: ticker.clone(),
            quantity: decision.quantity.unwrap(),
            price: quote.last_price,
        })
    }
    
    fn build_order(&self, decision: &LlmDecision, quote: &Quote) -> Result<Order> {
        // Build Schwab order from decision
    }
}
```

### 8. Main Loop (`src/main.rs`)

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();
    
    // Load config
    dotenvy::dotenv().ok();
    let settings = Settings::load()?;
    
    // Initialize components
    let schwab = Arc::new(SchwabClient::new(&settings.schwab).await?);
    let anthropic = AnthropicClient::new(&settings.anthropic);
    let quote_service = QuoteService::new(schwab.clone());
    let historical_service = HistoricalDataService::new(schwab.clone());
    let news_service = NewsService::new(&settings.data_sources);
    let sentiment_service = SentimentService::new(&settings.data_sources);
    
    let mut executor = TradeExecutor::new(
        schwab.clone(),
        TradingRules::new(&settings.trading),
        settings.schwab.account_id.clone(),
    );
    
    // Define watchlist
    let watchlist = vec!["AAPL", "GOOGL", "MSFT", "AMZN", "NVDA", "SPY", "QQQ"];
    
    // Main trading loop (runs during market hours)
    let mut interval = tokio::time::interval(Duration::from_secs(
        settings.data_sources.quote_interval_secs
    ));
    
    loop {
        interval.tick().await;
        
        // Check if market is open
        if !is_market_open() {
            tracing::debug!("Market closed, skipping cycle");
            continue;
        }
        
        // Gather data
        let quotes = quote_service.get_current_quotes(&watchlist).await?;
        let historical = historical_service.get_recent_bars(&watchlist).await?;
        let news = news_service.get_news(&watchlist, 10).await?;
        let sentiment = sentiment_service.get_sentiment_scores(&watchlist).await?;
        
        // Sanitize and structure
        let sanitized_news = InputSanitizer::sanitize_news(&news);
        let context = InputSanitizer::structure_market_context(
            &quotes,
            &historical,
            &sentiment,
            &sanitized_news,
        );
        
        // Get current portfolio
        let portfolio = schwab.get_portfolio(&settings.schwab.account_id).await?;
        
        // Build constraints for LLM
        let constraints = executor.rules.get_current_constraints(&portfolio);
        
        // Get LLM decision
        let prompt = PromptBuilder::build_trading_prompt(&context, &portfolio, &constraints);
        let response = anthropic.complete(&prompt).await?;
        
        // Parse response
        let decision = match ResponseParser::parse(&response) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to parse LLM response: {e}");
                continue;
            }
        };
        
        tracing::info!(
            action = ?decision.action,
            ticker = ?decision.ticker,
            reasoning = %decision.reasoning,
            "LLM decision"
        );
        
        // Execute (rules enforced inside)
        match executor.execute(decision, &portfolio).await {
            Ok(result) => {
                tracing::info!(?result, "Execution complete");
            }
            Err(e) => {
                tracing::warn!("Execution failed: {e}");
            }
        }
    }
}

fn is_market_open() -> bool {
    let now = Utc::now();
    let eastern = now.with_timezone(&chrono_tz::America::New_York);
    
    // Check weekday
    let weekday = eastern.weekday();
    if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
        return false;
    }
    
    // Check time (9:30 AM - 4:00 PM ET)
    let time = eastern.time();
    let open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();
    
    time >= open && time < close
    
    // Note: This doesn't handle holidays. Consider using a market calendar API.
}
```

### 9. Audit Logging (`src/logging/audit.rs`)

```rust
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
    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        
        let line = serde_json::to_string(&entry)? + "\n";
        file.write_all(line.as_bytes()).await?;
        
        Ok(())
    }
}
```

## Configuration Files

### settings.toml

```toml
[schwab]
redirect_uri = "http://localhost:8080/callback"
# account_id loaded from env

[anthropic]
model = "claude-sonnet-4-20250514"
max_tokens = 1024

[trading]
max_position_size_dollars = 100
max_position_pct = 20
max_daily_trades = 10
max_daily_loss_dollars = 50
day_trade_limit = 3
blocked_tickers = ["GME", "AMC", "BBBY"]

[data_sources]
quote_interval_secs = 60
```

### .env.example

```
SCHWAB_CLIENT_ID=your_client_id
SCHWAB_CLIENT_SECRET=your_client_secret
SCHWAB_ACCOUNT_ID=your_account_number

ANTHROPIC_API_KEY=sk-ant-...

NEWS_API_KEY=optional_news_api_key
SENTIMENT_API_KEY=optional_sentiment_api_key

RUST_LOG=info,schwab_trading_bot=debug
```

## Testing Strategy

### Unit Tests
- `rules.rs`: Test each rule violation scenario
- `sanitizer.rs`: Test injection pattern removal
- `response.rs`: Test JSON parsing edge cases

### Integration Tests
- Mock Schwab API responses with `wiremock`
- Test full decision -> validation -> execution flow
- Test token refresh flow

### Paper Trading
Before using real money:
1. Add a `dry_run` mode that logs orders but doesn't submit them
2. Run for at least a week monitoring decisions
3. Verify rules are enforced correctly

## Security Checklist

- [ ] Secrets loaded from environment, never committed
- [ ] OAuth tokens stored encrypted or in system keyring
- [ ] All text inputs sanitized before LLM
- [ ] LLM output parsed as structured data, not executed
- [ ] Trading rules enforced in Rust, not in prompts
- [ ] Audit log captures all decisions and orders
- [ ] Rate limiting on API calls
- [ ] TLS verification enabled (default in reqwest)

## Future Enhancements

1. **Streaming quotes**: Use Schwab's WebSocket feed for real-time data
2. **Multiple strategies**: Run different LLM prompts for different market conditions
3. **Backtesting**: Replay historical data through the decision engine
4. **Dashboard**: Web UI showing positions, P&L, decision history
5. **Alerting**: Push notifications on trades or rule violations
