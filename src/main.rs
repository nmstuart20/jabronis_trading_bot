use chrono::{Datelike, NaiveTime};
use clap::{Parser, Subcommand, ValueEnum};
use schwab_bot::config::Settings;
use schwab_bot::data::historical::HistoricalDataService;
use schwab_bot::data::news::NewsService;
use schwab_bot::data::quotes::QuoteService;
use schwab_bot::data::sentiment::SentimentService;
use schwab_bot::llm::client::AnthropicClient;
use schwab_bot::llm::prompts::PromptBuilder;
use schwab_bot::llm::response::ResponseParser;
use schwab_bot::llm::sanitizer::InputSanitizer;
use schwab_bot::logging::audit::{AuditEntry, AuditEventType, AuditLogger};
use schwab_bot::schwab::client::SchwabClient;
use schwab_bot::trading::executor::TradeExecutor;
use schwab_bot::trading::portfolio::Portfolio;
use schwab_bot::trading::rules::TradingRules;
use schwab_bot::trading::state::TradingState;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "schwab-bot",
    about = "LLM-powered trading bot with Schwab integration"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single trading session
    Trade {
        /// Trading mode
        #[arg(short, long, default_value = "manual")]
        mode: TradingMode,
        /// Force dry-run mode (overrides config)
        #[arg(long)]
        dry_run: bool,
    },
    /// Run the OAuth2 authentication flow
    Auth,
    /// Show current portfolio status
    Status,
    /// Show recent trade history
    History {
        /// Number of days to show
        #[arg(short, long, default_value = "7")]
        days: u32,
    },
}

#[derive(Clone, ValueEnum)]
enum TradingMode {
    Open,
    Midday,
    Preclose,
    Manual,
}

impl TradingMode {
    fn as_str(&self) -> &str {
        match self {
            TradingMode::Open => "open",
            TradingMode::Midday => "midday",
            TradingMode::Preclose => "preclose",
            TradingMode::Manual => "manual",
        }
    }
}

fn is_market_open() -> bool {
    let now = chrono::Utc::now();
    let eastern = now.with_timezone(&chrono_tz::America::New_York);

    let weekday = eastern.weekday();
    if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
        return false;
    }

    let time = eastern.time();
    let open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();

    time >= open && time < close
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Trade { mode, dry_run } => run_trade_session(mode, dry_run).await,
        Commands::Auth => run_auth_flow().await,
        Commands::Status => show_status().await,
        Commands::History { days } => show_history(days).await,
    }
}

async fn run_trade_session(mode: TradingMode, force_dry_run: bool) -> anyhow::Result<()> {
    let settings = Settings::load()?;

    // Check market hours unless manual mode
    if !matches!(mode, TradingMode::Manual) && !is_market_open() {
        println!("Market is closed. Use --mode manual to run anyway.");
        return Ok(());
    }

    // Load persistent state
    let mut state = TradingState::load_or_create(&settings.state_path)?;

    let audit = AuditLogger::new(settings.logging.audit_path.clone());
    let schwab = Arc::new(SchwabClient::new(&settings.schwab).await?);

    if let Err(e) = schwab.ensure_authenticated().await {
        tracing::warn!("Initial auth failed (may need browser auth): {e}");
    }

    let anthropic = AnthropicClient::new(&settings.anthropic);
    let quote_service = QuoteService::new(schwab.clone());
    let historical_service = HistoricalDataService::new(schwab.clone());
    let news_service = NewsService::new(&settings.data_sources);
    let sentiment_service = SentimentService::new(&settings.data_sources);

    let mut rules = TradingRules::new(&settings.trading);
    rules.load_history(state.trade_history.clone(), state.daily_pnl);
    if force_dry_run {
        rules.set_dry_run(true);
    }

    let mut executor = TradeExecutor::new(schwab.clone(), rules);

    let watchlist: Vec<&str> = settings
        .trading
        .watchlist
        .iter()
        .map(|s| s.as_str())
        .collect();

    let dry_run_active = force_dry_run || settings.trading.dry_run;
    println!(
        "Running trade session (mode: {}, dry_run: {})",
        mode.as_str(),
        dry_run_active
    );

    // Gather data
    let quotes = quote_service.get_current_quotes(&watchlist).await?;
    let historical = historical_service
        .get_recent_bars(&watchlist)
        .await
        .unwrap_or_default();
    let news = news_service
        .get_news(&watchlist, 10)
        .await
        .unwrap_or_default();
    let sentiment = sentiment_service
        .get_sentiment_scores(&watchlist)
        .await
        .unwrap_or_default();

    // Sanitize and structure
    let sanitized_news = InputSanitizer::sanitize_news(&news);
    let context =
        InputSanitizer::structure_market_context(&quotes, &historical, &sentiment, &sanitized_news);

    // Build portfolio
    let portfolio = build_portfolio(&schwab).await?;

    // Build constraints
    let constraints = executor.rules.get_current_constraints(&portfolio);

    // Get LLM decision
    let prompt =
        PromptBuilder::build_trading_prompt(&context, &portfolio, &constraints, mode.as_str());

    let _ = audit
        .log(AuditEntry {
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::LlmRequest,
            details: serde_json::json!({"prompt": &prompt, "prompt_length": prompt.len(), "mode": mode.as_str()}),
        })
        .await;

    let response = match anthropic.complete(&prompt).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("LLM request failed: {e}");
            let _ = audit
                .log(AuditEntry {
                    timestamp: chrono::Utc::now(),
                    event_type: AuditEventType::Error,
                    details: serde_json::json!({"error": e.to_string()}),
                })
                .await;
            return Err(e.into());
        }
    };

    let _ = audit
        .log(AuditEntry {
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::LlmResponse,
            details: serde_json::json!({"response": &response, "response_length": response.len()}),
        })
        .await;

    // Parse response as a plan (supports single action or SELL+BUY pair)
    let plan = ResponseParser::parse_plan(&response)?;

    for (i, action) in plan.actions.iter().enumerate() {
        println!(
            "Action {}: {:?} {} ({})",
            i + 1,
            action.action,
            action.ticker.as_deref().unwrap_or("-"),
            action.reasoning
        );
    }

    let _ = audit
        .log(AuditEntry {
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::TradeDecision,
            details: serde_json::json!({
                "plan_size": plan.actions.len(),
                "actions": plan.actions.iter().map(|a| serde_json::json!({
                    "action": format!("{:?}", a.action),
                    "ticker": a.ticker,
                    "quantity": a.quantity,
                    "reasoning": a.reasoning,
                })).collect::<Vec<_>>(),
            }),
        })
        .await;

    // Execute plan (sequentially: SELL first if needed, then BUY)
    match executor.execute_plan(plan, &portfolio).await {
        Ok(results) => {
            for result in &results {
                println!("Result: {result:?}");
            }
            let _ = audit
                .log(AuditEntry {
                    timestamp: chrono::Utc::now(),
                    event_type: AuditEventType::OrderSubmitted,
                    details: serde_json::json!({"results": results.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>()}),
                })
                .await;
        }
        Err(e) => {
            println!("Execution failed: {e}");
            let _ = audit
                .log(AuditEntry {
                    timestamp: chrono::Utc::now(),
                    event_type: AuditEventType::RuleViolation,
                    details: serde_json::json!({"error": e.to_string()}),
                })
                .await;
        }
    }

    // Save state back
    state.trade_history = executor.rules.trade_history().to_vec();
    state.daily_pnl = executor.rules.daily_pnl();
    state.save(&settings.state_path)?;
    println!("State saved to {}", settings.state_path.display());

    Ok(())
}

async fn run_auth_flow() -> anyhow::Result<()> {
    let settings = Settings::load()?;
    let schwab = SchwabClient::new(&settings.schwab).await?;
    schwab.ensure_authenticated().await?;
    println!("Authentication successful.");
    Ok(())
}

async fn show_status() -> anyhow::Result<()> {
    let settings = Settings::load()?;
    let schwab = Arc::new(SchwabClient::new(&settings.schwab).await?);

    if let Err(e) = schwab.ensure_authenticated().await {
        println!("Auth failed: {e}");
        return Err(e.into());
    }

    let portfolio = build_portfolio(&schwab).await?;

    println!("=== Portfolio Status ===");
    println!("Cash Available: ${}", portfolio.cash_available);
    println!("Total Value:    ${}", portfolio.total_account_value);
    println!();

    if portfolio.positions.is_empty() {
        println!("No open positions.");
    } else {
        println!(
            "{:<8} {:>8} {:>10} {:>12} {:>10}",
            "Symbol", "Qty", "Avg Price", "Value", "P&L"
        );
        println!("{}", "-".repeat(52));
        for p in &portfolio.positions {
            println!(
                "{:<8} {:>8} {:>10} {:>12} {:>10}",
                p.symbol, p.quantity, p.average_price, p.current_value, p.unrealized_pnl
            );
        }
    }

    // Show state info if available
    if let Ok(state) = TradingState::load_or_create(&settings.state_path) {
        let today_trades: Vec<_> = state
            .trade_history
            .iter()
            .filter(|t| t.timestamp.date_naive() == chrono::Utc::now().date_naive())
            .collect();
        println!();
        println!("Trades today: {}", today_trades.len());
        println!("Daily P&L:    ${}", state.daily_pnl);
    }

    Ok(())
}

async fn show_history(days: u32) -> anyhow::Result<()> {
    let settings = Settings::load()?;
    let state = TradingState::load_or_create(&settings.state_path)?;

    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let recent: Vec<_> = state
        .trade_history
        .iter()
        .filter(|t| t.timestamp >= cutoff)
        .collect();

    if recent.is_empty() {
        println!("No trades in the last {days} days.");
        return Ok(());
    }

    println!("=== Trade History (last {days} days) ===");
    println!(
        "{:<20} {:<6} {:<8} {:>6} {:>10} {:<12}",
        "Timestamp", "Action", "Ticker", "Qty", "Price", "Order ID"
    );
    println!("{}", "-".repeat(66));
    for t in &recent {
        println!(
            "{:<20} {:<6} {:<8} {:>6} {:>10} {:<12}",
            t.timestamp.format("%Y-%m-%d %H:%M:%S"),
            format!("{:?}", t.action),
            t.ticker,
            t.quantity,
            t.price,
            t.order_id,
        );
    }

    Ok(())
}

async fn build_portfolio(schwab: &SchwabClient) -> schwab_bot::error::Result<Portfolio> {
    let account = schwab.get_account().await?;
    let positions = schwab.get_positions().await?;

    let balances = account
        .securities_account
        .and_then(|sa| sa.current_balances)
        .unwrap_or(schwab_bot::schwab::models::AccountBalances {
            cash_available_for_trading: rust_decimal::Decimal::ZERO,
            liquidation_value: rust_decimal::Decimal::ZERO,
        });

    Ok(Portfolio {
        cash_available: balances.cash_available_for_trading,
        positions,
        total_account_value: balances.liquidation_value,
    })
}
