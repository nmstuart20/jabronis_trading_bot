# Schwab Trading Bot

LLM-powered trading bot that uses Claude as an advisory-only signal. All trading rules are enforced in Rust — the LLM can suggest any action, but rule enforcement gates execution.

Dry-run mode is enabled by default. No real orders are placed unless you explicitly disable it in config.

## Setup

1. Copy `.env.example` to `.env` and fill in your credentials:

```
SCHWAB_CLIENT_ID=...
SCHWAB_CLIENT_SECRET=...
SCHWAB_ACCOUNT_ID=...
ANTHROPIC_API_KEY=...
```

2. Build the project:

```bash
cargo build --release
```

## Usage

```bash
# Authenticate with Schwab (opens browser for OAuth2 flow)
schwab_bot auth

# Run a single trading session (manual mode, dry-run forced)
schwab_bot trade --mode manual --dry-run

# Run with a specific market session mode
schwab_bot trade --mode open
schwab_bot trade --mode midday
schwab_bot trade --mode preclose

# View current portfolio status
schwab_bot status

# View trade history (last 7 days by default)
schwab_bot history
schwab_bot history --days 30
```

### Trading Modes

| Mode | Description |
|------|-------------|
| `open` | Focuses on opening momentum and gap analysis. Requires market hours. |
| `midday` | Focuses on trend continuation/reversal signals. Requires market hours. |
| `preclose` | Conservative — avoids new large positions near close. Requires market hours. |
| `manual` | Runs regardless of market hours. Default mode. |

### Flags

- `--dry-run` on the `trade` command forces dry-run mode, overriding the config file setting.
- `--days N` on the `history` command controls how far back to show trades.

## Configuration

Edit `config/settings.toml` to adjust trading rules, position limits, and other settings. Key options:

- `trading.dry_run` — `true` by default, set to `false` to enable live trading
- `trading.max_position_size_dollars` — max dollar value per position
- `trading.max_daily_trades` — max trades per day
- `trading.max_daily_loss_dollars` — daily loss circuit breaker
- `trading.blocked_tickers` — tickers the bot will never trade
- `state_path` — path to persistent state file (default: `data/state.json`)

## Running on a Schedule

This bot is designed to be invoked by an external scheduler (e.g., cron) rather than running as a continuous process:

```cron
# Market open (9:35 AM ET)
35 9 * * 1-5 cd /path/to/schwab_bot && ./target/release/schwab_bot trade --mode open

# Midday (12:00 PM ET)
0 12 * * 1-5 cd /path/to/schwab_bot && ./target/release/schwab_bot trade --mode midday

# Pre-close (3:45 PM ET)
45 15 * * 1-5 cd /path/to/schwab_bot && ./target/release/schwab_bot trade --mode preclose
```

## Development

```bash
cargo test                                    # Run all tests
cargo clippy --all-targets --all-features     # Lint
cargo fmt                                     # Format
RUST_LOG=debug cargo run -- trade --mode manual --dry-run  # Run with debug logging
```
