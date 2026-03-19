use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;

use schwab_bot::llm::response::Action;
use schwab_bot::trading::rules::TradeRecord;

// ---------------------------------------
// Schwab API Integration Tests (requires real credentials + browser auth)
// ---------------------------------------

mod schwab_integration {
    use schwab_bot::config::SchwabConfig;
    use schwab_bot::schwab::client::SchwabClient;
    use secrecy::SecretString;

    /// Integration test that authenticates with Schwab and fetches account holdings.
    ///
    /// Run with: cargo test schwab_integration::get_holdings -- --ignored --nocapture
    ///
    /// Requires:
    ///   - SCHWAB__APP_KEY and SCHWAB__APP_SECRET env vars
    ///   - SCHWAB__REDIRECT_URI env var
    ///   - A browser to complete the OAuth flow
    #[tokio::test]
    #[ignore]
    async fn get_holdings() {
        dotenvy::dotenv().ok();

        let app_key = std::env::var("SCHWAB__APP_KEY").expect("SCHWAB__APP_KEY env var required");
        let app_secret =
            std::env::var("SCHWAB__APP_SECRET").expect("SCHWAB__APP_SECRET env var required");
        let redirect_uri =
            std::env::var("SCHWAB__REDIRECT_URI").expect("SCHWAB__REDIRECT_URI env var required");

        let config = SchwabConfig {
            app_key: SecretString::from(app_key),
            app_secret: SecretString::from(app_secret),
            redirect_uri,
        };

        let client = SchwabClient::new(&config)
            .await
            .expect("Failed to create client");

        // This opens a browser for OAuth — complete the login there
        client
            .ensure_authenticated()
            .await
            .expect("Authentication failed");

        // Fetch linked account numbers
        let account_numbers = client
            .get_account_numbers()
            .await
            .expect("Failed to get account numbers");
        assert!(!account_numbers.is_empty(), "No linked accounts found");
        println!("\n=== Linked Accounts ===");
        for acct in &account_numbers {
            println!(
                "  Account: {} (hash: {}...)",
                acct.account_number,
                &acct.hash_value[..8.min(acct.hash_value.len())]
            );
        }

        // Fetch account details with positions
        let account = client
            .get_account()
            .await
            .expect("Failed to get account details");
        let securities = account
            .securities_account
            .expect("No securities account in response");

        if let Some(balances) = &securities.current_balances {
            println!("\n=== Balances ===");
            println!("  Cash Available: ${}", balances.cash_available_for_trading);
            println!("  Liquidation Value: ${}", balances.liquidation_value);
        }

        // Fetch positions via the client helper (tests SchwabPosition -> Position conversion)
        let positions = client
            .get_positions()
            .await
            .expect("Failed to get positions");

        println!("\n=== Positions ({}) ===", positions.len());
        if positions.is_empty() {
            println!("  No open positions.");
        } else {
            println!(
                "  {:<8} {:>10} {:>12} {:>12} {:>10}",
                "Symbol", "Qty", "Avg Price", "Value", "Day P&L"
            );
            println!("  {}", "-".repeat(56));
            for p in &positions {
                println!(
                    "  {:<8} {:>10} {:>12} {:>12} {:>10}",
                    p.symbol, p.quantity, p.average_price, p.current_value, p.unrealized_pnl
                );
            }
        }

        // Basic sanity: the API returned valid data
        if let Some(balances) = &securities.current_balances {
            assert!(
                balances.liquidation_value >= rust_decimal::Decimal::ZERO,
                "Liquidation value should be non-negative"
            );
        }
    }

    /// Integration test that previews an order through Schwab's API without placing it.
    /// Validates that order construction and the preview endpoint work end-to-end.
    ///
    /// Run with: cargo test schwab_integration::preview_order -- --ignored --nocapture
    ///
    /// Requires:
    ///   - SCHWAB__APP_KEY and SCHWAB__APP_SECRET env vars
    ///   - SCHWAB__REDIRECT_URI env var
    ///   - A browser to complete the OAuth flow
    #[tokio::test]
    #[ignore]
    async fn preview_order() {
        use rust_decimal::Decimal;
        use schwab_bot::schwab::orders;
        use std::str::FromStr;

        dotenvy::dotenv().ok();

        let app_key = std::env::var("SCHWAB__APP_KEY")
            .expect("SCHWAB__APP_KEY env var required");
        let app_secret = std::env::var("SCHWAB__APP_SECRET")
            .expect("SCHWAB__APP_SECRET env var required");
        let redirect_uri = std::env::var("SCHWAB__REDIRECT_URI")
            .expect("SCHWAB__REDIRECT_URI env var required");

        let config = SchwabConfig {
            app_key: SecretString::from(app_key),
            app_secret: SecretString::from(app_secret),
            redirect_uri,
        };

        let client = SchwabClient::new(&config)
            .await
            .expect("Failed to create client");

        client
            .ensure_authenticated()
            .await
            .expect("Authentication failed");

        // Preview a small limit buy far below market — will never fill
        let order = orders::build_limit_buy("OCUL", 1, Decimal::from_str("8.80").unwrap());
        let preview = client
            .preview_order(&order)
            .await
            .expect("Preview order failed");

        println!("\n=== Order Preview ===");
        if let Some(strategy) = &preview.order_strategy {
            println!("  Status: {:?}", strategy.status);
            for leg in &strategy.order_legs {
                println!(
                    "  Leg: {} {} @ bid={:?} ask={:?} last={:?}, commission={:?}",
                    leg.instruction.as_deref().unwrap_or("?"),
                    leg.final_symbol.as_deref().unwrap_or("?"),
                    leg.bid_price,
                    leg.ask_price,
                    leg.last_price,
                    leg.projected_commission,
                );
            }
        }
        if let Some(validation) = &preview.order_validation_result {
            if !validation.accepts.is_empty() {
                println!("  Accepts:");
                for msg in &validation.accepts {
                    println!("    - {}", msg.message.as_deref().unwrap_or("(no message)"));
                }
            }
            if !validation.warns.is_empty() {
                println!("  Warnings:");
                for msg in &validation.warns {
                    println!("    - {}", msg.message.as_deref().unwrap_or("(no message)"));
                }
            }
            if !validation.rejects.is_empty() {
                println!("  Rejects:");
                for msg in &validation.rejects {
                    println!("    - {}", msg.message.as_deref().unwrap_or("(no message)"));
                }
            }
            // A limit buy at $1 for AAPL should be accepted (just won't fill)
            assert!(
                validation.rejects.is_empty(),
                "Order was rejected: {:?}",
                validation.rejects
            );
        }
    }
}

// ---------------------------------------
// State Persistence Tests
// ---------------------------------------

mod state_persistence {
    use super::*;
    use schwab_bot::trading::state::TradingState;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn temp_state_path() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        (dir, path)
    }

    #[test]
    fn load_or_create_no_file() {
        let (_dir, path) = temp_state_path();
        let state = TradingState::load_or_create(&path).unwrap();
        assert!(state.trade_history.is_empty());
        assert_eq!(state.daily_pnl, Decimal::ZERO);
        assert_eq!(state.last_reset_date, Utc::now().date_naive());
    }

    #[test]
    fn save_and_load_round_trip() {
        let (_dir, path) = temp_state_path();
        let mut state = TradingState::load_or_create(&path).unwrap();

        state.trade_history.push(TradeRecord {
            timestamp: Utc::now(),
            ticker: "AAPL".into(),
            action: Action::Buy,
            quantity: 10,
            price: Decimal::from_str("150").unwrap(),
            order_id: "test-1".into(),
        });
        state.daily_pnl = Decimal::from_str("42.50").unwrap();
        state.save(&path).unwrap();

        let loaded = TradingState::load_or_create(&path).unwrap();
        assert_eq!(loaded.trade_history.len(), 1);
        assert_eq!(loaded.trade_history[0].ticker, "AAPL");
        assert_eq!(loaded.daily_pnl, Decimal::from_str("42.50").unwrap());
    }

    #[test]
    fn daily_pnl_resets_on_new_day() {
        let (_dir, path) = temp_state_path();
        let state = TradingState {
            trade_history: vec![],
            daily_pnl: Decimal::from_str("-100").unwrap(),
            last_reset_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        };
        state.save(&path).unwrap();

        let loaded = TradingState::load_or_create(&path).unwrap();
        assert_eq!(loaded.daily_pnl, Decimal::ZERO);
        assert_eq!(loaded.last_reset_date, Utc::now().date_naive());
    }

    #[test]
    fn old_trades_pruned() {
        let (_dir, path) = temp_state_path();
        let old_time = Utc::now() - ChronoDuration::days(60);
        let recent_time = Utc::now() - ChronoDuration::days(5);

        let state = TradingState {
            trade_history: vec![
                TradeRecord {
                    timestamp: old_time,
                    ticker: "OLD".into(),
                    action: Action::Buy,
                    quantity: 1,
                    price: Decimal::from_str("100").unwrap(),
                    order_id: "old-1".into(),
                },
                TradeRecord {
                    timestamp: recent_time,
                    ticker: "NEW".into(),
                    action: Action::Buy,
                    quantity: 1,
                    price: Decimal::from_str("200").unwrap(),
                    order_id: "new-1".into(),
                },
            ],
            daily_pnl: Decimal::ZERO,
            last_reset_date: Utc::now().date_naive(),
        };
        state.save(&path).unwrap();

        let loaded = TradingState::load_or_create(&path).unwrap();
        assert_eq!(loaded.trade_history.len(), 1);
        assert_eq!(loaded.trade_history[0].ticker, "NEW");
    }
}

// ---------------------------------------
// News API Integration Tests (requires NEWS_API_KEY env var)
// ---------------------------------------

mod news_integration {
    use schwab_bot::config::DataSourcesConfig;
    use schwab_bot::data::news::NewsService;
    use secrecy::SecretString;

    /// Integration test that fetches real news from the NewsAPI.
    ///
    /// Run with: cargo test news_integration::fetch_news -- --ignored --nocapture
    ///
    /// Requires:
    ///   - DATA_SOURCES__NEWS_API_KEY env var (get one at https://newsapi.org)
    #[tokio::test]
    #[ignore]
    async fn fetch_news() {
        dotenvy::dotenv().ok();

        let api_key = std::env::var("DATA_SOURCES__NEWS_API_KEY")
            .expect("DATA_SOURCES__NEWS_API_KEY env var required");

        let config = DataSourcesConfig {
            news_api_key: Some(SecretString::from(api_key)),
            sentiment_api_key: None,
            quote_interval_secs: 10,
        };

        let service = NewsService::new(&config);
        let articles = service
            .get_news(&["AAPL", "MSFT"], 5)
            .await
            .expect("Failed to fetch news");

        assert!(!articles.is_empty(), "Expected at least one news article");

        println!("\n=== News Articles ({}) ===", articles.len());
        for article in &articles {
            println!(
                "  [{}] {} - {}",
                article.source, article.headline, article.published_at
            );
            if !article.summary.is_empty() {
                println!("    {}", &article.summary[..article.summary.len().min(100)]);
            }
        }

        // Verify article fields are populated
        let first = &articles[0];
        assert!(!first.headline.is_empty(), "Headline should not be empty");
        assert!(!first.source.is_empty(), "Source should not be empty");
        assert_eq!(first.symbols, vec!["AAPL", "MSFT"]);
    }

    /// Verifies that missing API key returns empty results (no error).
    #[tokio::test]
    async fn no_api_key_returns_empty() {
        let config = DataSourcesConfig {
            news_api_key: None,
            sentiment_api_key: None,
            quote_interval_secs: 10,
        };

        let service = NewsService::new(&config);
        let articles = service
            .get_news(&["AAPL"], 5)
            .await
            .expect("Should not error without API key");

        assert!(
            articles.is_empty(),
            "Should return empty vec without API key"
        );
    }
}
