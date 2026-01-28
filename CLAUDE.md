# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo test                           # Run all tests
cargo test trading_rules::           # Run tests matching a pattern
cargo test --test unit_tests         # Run integration test file
cargo fmt                            # Format code
cargo clippy --all-targets --all-features  # Lint
RUST_LOG=debug cargo run             # Run with debug logging
```

## Architecture

LLM-powered trading bot that uses Claude as an **advisory-only** signal — all trading rules are enforced in Rust, never delegated to the LLM. Defense-in-depth: the LLM can suggest any action, but `trading::rules` gates execution.

### Module Layout

- **`schwab/`** — OAuth2 PKCE auth (`auth.rs`), Schwab Trader API client (`client.rs`), order builders (`orders.rs`), API data models (`models.rs`)
- **`data/`** — Market data services: real-time quotes with DashMap caching (10s TTL), historical OHLCV bars, NewsAPI integration, sentiment scoring (placeholder)
- **`llm/`** — Anthropic API client, prompt construction (`prompts.rs`), input sanitization against prompt injection (`sanitizer.rs`), JSON response parsing with validation (`response.rs`)
- **`trading/`** — Rule enforcement engine (`rules.rs`), trade executor (`executor.rs`), portfolio state (`portfolio.rs`), constraint/result types (`decision.rs`)
- **`logging/`** — JSON audit trail logging every LLM request/response, decision, order, and rule violation

### Main Loop (`main.rs`)

Polls on a configurable interval during market hours (Mon–Fri 9:30–16:00 ET): fetch data → sanitize → build prompt → call Claude → parse JSON response → validate against rules → execute or dry-run → audit log.

### Key Design Decisions

- **`rust_decimal::Decimal`** for all monetary values (no floating point)
- **`secrecy::SecretString`** for all API keys and credentials (prevents accidental logging)
- **Dry-run mode enabled by default** (`trading.dry_run = true` in config) — orders are logged but not submitted
- **Sanitizer** strips injection keywords and angle brackets/braces from news/sentiment before prompt construction
- **LLM response format**: JSON with `action` (BUY/SELL/HOLD), `ticker`, `quantity`, `order_type`, `limit_price`, `reasoning`
- **Trading rules** enforce: position size limits ($ and %), PDT day-trade limit, daily loss cap, daily trade count, ticker whitelist/blacklist, cash sufficiency

## Configuration

Settings loaded from `config/settings.toml` with secrets from environment variables (see `.env.example`). Key env vars: `SCHWAB_CLIENT_ID`, `SCHWAB_CLIENT_SECRET`, `SCHWAB_ACCOUNT_ID`, `ANTHROPIC_API_KEY`.

## Tests

All tests live in `tests/unit_tests.rs` as integration tests. Test modules cover: response parsing, input sanitization, trading rules, order builders, portfolio, and model serialization. Helper functions (`make_trading_config`, `make_portfolio`, `make_quote`, etc.) are defined at the top of the test file.
