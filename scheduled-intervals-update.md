# Update: Convert from Continuous Polling to Scheduled Intervals

The current implementation uses a continuous polling loop that runs every N seconds during market hours. We need to convert this to a CLI-based approach that runs 3x/day via cron or systemd.

## Why This Change

- Continuous polling burns ~390 LLM API calls/day (every minute for 6.5 hours)
- Most cycles return "HOLD" with no action
- Scheduled intervals (3 calls/day) are more cost-effective for a small experimental account
- State needs to persist between invocations

## Changes Required

### 1. Add CLI with clap

Add `clap = { version = "4", features = ["derive"] }` to Cargo.toml.

Replace the main loop with a CLI structure:

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "schwab-bot")]
#[command(about = "LLM-powered trading bot for Schwab")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a trading session
    Trade {
        #[arg(short, long)]
        mode: TradingMode,
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },
    /// Authenticate with Schwab (first-time setup)
    Auth,
    /// Show current portfolio and state
    Status,
    /// Show trade history
    History {
        #[arg(short, long, default_value = "7")]
        days: u32,
    },
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
enum TradingMode {
    /// Market open (9:30 AM ET) - React to overnight news
    Open,
    /// Mid-day (12:00 PM ET) - Check positions
    Midday,
    /// Pre-close (3:30 PM ET) - Decide on overnight holds
    Preclose,
    /// Manual run for testing
    Manual,
}
```

### 2. Add State Persistence

Create `src/trading/state.rs`:

```rust
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct TradingState {
    pub trade_history: Vec<TradeRecord>,
    pub daily_pnl: Decimal,
    pub last_reset_date: NaiveDate,
}

impl TradingState {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let mut state: TradingState = serde_json::from_str(&contents)?;
            
            // Reset daily P&L if new day
            let today = Utc::now().date_naive();
            if state.last_reset_date < today {
                state.daily_pnl = Decimal::ZERO;
                state.last_reset_date = today;
                state.prune_old_trades();
            }
            
            Ok(state)
        } else {
            Ok(Self {
                trade_history: Vec::new(),
                daily_pnl: Decimal::ZERO,
                last_reset_date: Utc::now().date_naive(),
            })
        }
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
    
    fn prune_old_trades(&mut self) {
        let cutoff = Utc::now() - chrono::Duration::days(7);
        self.trade_history.retain(|t| t.timestamp > cutoff);
    }
}
```

### 3. Add Mode-Specific Prompts

Update `src/llm/prompts.rs` to accept a `TradingMode` parameter and include mode-specific instructions:

```rust
pub fn build_trading_prompt(
    context: &MarketContext,
    portfolio: &Portfolio,
    constraints: &TradingConstraints,
    mode: &TradingMode,
) -> String {
    let mode_instructions = match mode {
        TradingMode::Open => r#"
## Session: Market Open
Focus on:
- Overnight news that moved futures or pre-market
- Gap ups/downs from previous close
- Setting up positions for the day
- Being cautious in first 15 minutes (high volatility)
"#,
        TradingMode::Midday => r#"
## Session: Mid-Day Check
Focus on:
- How morning positions are performing
- Whether to add to winners or cut losers
- Breaking news since market open
- Sector rotation or momentum shifts
"#,
        TradingMode::Preclose => r#"
## Session: Pre-Close
Market closes in 30 minutes. Focus on:
- Whether to hold positions overnight (earnings/weekend risk)
- Taking profits on day trades before close
- Closing positions you don't want overnight exposure on
- Holding overnight = no day trade counted
"#,
        TradingMode::Manual => r#"
## Session: Manual Override
Analyze current situation and recommend action.
"#,
    };
    
    // Include mode_instructions in the prompt template
    // ... rest of prompt building
}
```

### 4. Update main.rs

Replace the continuous loop with:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();
    
    let cli = Cli::parse();
    dotenvy::dotenv().ok();
    let settings = Settings::load()?;
    
    match cli.command {
        Commands::Trade { mode, dry_run } => {
            run_trade_session(&settings, mode, dry_run).await?;
        }
        Commands::Auth => {
            run_auth_flow(&settings).await?;
        }
        Commands::Status => {
            show_status(&settings).await?;
        }
        Commands::History { days } => {
            show_history(&settings, days)?;
        }
    }
    
    Ok(())
}

async fn run_trade_session(settings: &Settings, mode: TradingMode, dry_run: bool) -> Result<()> {
    // Check market hours (skip for Manual mode)
    if mode != TradingMode::Manual && !is_market_open() {
        tracing::info!("Market closed, exiting");
        return Ok(());
    }
    
    // Load persisted state
    let state_path = Path::new("data/state.json");
    let mut trading_state = TradingState::load_or_create(state_path)?;
    
    // Initialize clients
    let schwab = SchwabClient::new(&settings.schwab).await?;
    let anthropic = AnthropicClient::new(&settings.anthropic);
    // ... other services
    
    // Gather data, build context, get LLM decision
    // ... existing logic but pass `mode` to prompt builder
    
    // Execute if not dry run
    if dry_run {
        tracing::info!(?decision, "DRY RUN - would execute");
    } else {
        // Execute and record trade in trading_state
    }
    
    // Save state
    trading_state.save(state_path)?;
    
    Ok(())
}
```

### 5. Update Config

Add to settings.toml:
```toml
state_path = "data/state.json"

[logging]
audit_path = "logs/audit.jsonl"
```

Add to Settings struct:
```rust
pub state_path: PathBuf,
pub logging: LoggingConfig,

pub struct LoggingConfig {
    pub audit_path: PathBuf,
}
```

### 6. Add chrono-tz for Timezone Support

Add `chrono-tz = "0.10"` to Cargo.toml for proper ET timezone handling in `is_market_open()`.

### 7. Create Directory Structure

Ensure these directories exist or are created on first run:
- `data/` - for state.json
- `logs/` - for audit.jsonl

## Scheduling (Documentation Only)

After building, schedule with cron (adjust for your timezone relative to ET):

```bash
# crontab -e
30 9  * * 1-5 /path/to/schwab-bot trade --mode open >> /var/log/schwab-bot.log 2>&1
0  12 * * 1-5 /path/to/schwab-bot trade --mode midday >> /var/log/schwab-bot.log 2>&1
30 15 * * 1-5 /path/to/schwab-bot trade --mode preclose >> /var/log/schwab-bot.log 2>&1
```

## Testing

1. Build: `cargo build --release`
2. Test auth: `./target/release/schwab-bot auth`
3. Dry run: `./target/release/schwab-bot trade --mode manual --dry-run`
4. Check status: `./target/release/schwab-bot status`
