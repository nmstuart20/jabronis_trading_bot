use crate::config::TradingConfig;
use crate::llm::response::{Action, LlmDecision};
use crate::schwab::models::Quote;
use crate::trading::decision::TradingConstraints;
use crate::trading::portfolio::Portfolio;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradeRecord {
    pub timestamp: DateTime<Utc>,
    pub ticker: String,
    pub action: Action,
    pub quantity: u32,
    pub price: Decimal,
    pub order_id: String,
}

pub struct TradingRules {
    config: TradingConfig,
    trade_history: Vec<TradeRecord>,
    daily_pnl: Decimal,
}

#[derive(Debug, thiserror::Error)]
pub enum RuleViolation {
    #[error("Ticker {0} is not allowed")]
    TickerNotAllowed(String),
    #[error("Position too large: ${requested}, max ${max}")]
    PositionTooLarge { requested: Decimal, max: Decimal },
    #[error("Position percentage too high: {requested_pct}%, max {max_pct}%")]
    PositionPercentageTooHigh {
        requested_pct: Decimal,
        max_pct: Decimal,
    },
    #[error("Day trade limit exceeded (PDT rule)")]
    DayTradeLimitExceeded,
    #[error("Daily loss limit reached")]
    DailyLossLimitReached,
    #[error("Daily trade count exceeded")]
    DailyTradeCountExceeded,
    #[error("Insufficient cash: need ${requested}, have ${available}")]
    InsufficientCash {
        requested: Decimal,
        available: Decimal,
    },
}

impl TradingRules {
    pub fn new(config: &TradingConfig) -> Self {
        Self {
            config: config.clone(),
            trade_history: Vec::new(),
            daily_pnl: Decimal::ZERO,
        }
    }

    pub fn validate_trade(
        &self,
        decision: &LlmDecision,
        portfolio: &Portfolio,
        quote: &Quote,
    ) -> Result<(), RuleViolation> {
        if let Some(ticker) = &decision.ticker {
            self.check_ticker_allowed(ticker)?;
        }
        self.check_position_size(decision, portfolio, quote)?;
        self.check_day_trade_limit(decision)?;
        self.check_daily_loss_limit()?;
        self.check_daily_trade_count()?;
        Ok(())
    }

    fn check_ticker_allowed(&self, ticker: &str) -> Result<(), RuleViolation> {
        if self
            .config
            .blocked_tickers
            .iter()
            .any(|b| b.eq_ignore_ascii_case(ticker))
        {
            return Err(RuleViolation::TickerNotAllowed(ticker.to_string()));
        }
        if let Some(allowed) = &self.config.allowed_tickers {
            if !allowed.iter().any(|a| a.eq_ignore_ascii_case(ticker)) {
                return Err(RuleViolation::TickerNotAllowed(ticker.to_string()));
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
        let quantity = Decimal::from(decision.quantity.unwrap_or(0));
        let trade_value = quantity * quote.ask_price;

        if trade_value > self.config.max_position_size_dollars {
            return Err(RuleViolation::PositionTooLarge {
                requested: trade_value,
                max: self.config.max_position_size_dollars,
            });
        }

        let portfolio_value = portfolio.total_value();
        if portfolio_value > Decimal::ZERO {
            let pct = trade_value / portfolio_value * Decimal::from(100);
            if pct > self.config.max_position_pct {
                return Err(RuleViolation::PositionPercentageTooHigh {
                    requested_pct: pct,
                    max_pct: self.config.max_position_pct,
                });
            }
        }

        if trade_value > portfolio.cash_available {
            return Err(RuleViolation::InsufficientCash {
                requested: trade_value,
                available: portfolio.cash_available,
            });
        }
        Ok(())
    }

    fn check_day_trade_limit(&self, decision: &LlmDecision) -> Result<(), RuleViolation> {
        if decision.action != Action::Sell {
            return Ok(());
        }
        if let Some(ticker) = &decision.ticker {
            let today = Utc::now().date_naive();
            let bought_today = self.trade_history.iter().any(|t| {
                t.ticker == *ticker && t.action == Action::Buy && t.timestamp.date_naive() == today
            });
            if bought_today {
                let day_trades = self.count_day_trades_rolling_5_days();
                if day_trades >= self.config.day_trade_limit {
                    return Err(RuleViolation::DayTradeLimitExceeded);
                }
            }
        }
        Ok(())
    }

    fn count_day_trades_rolling_5_days(&self) -> u32 {
        let now = Utc::now();
        let five_days_ago = now - chrono::Duration::days(5);
        let mut day_trade_count = 0u32;

        // Find sell records in the last 5 days where we also bought same ticker same day
        for trade in &self.trade_history {
            if trade.timestamp < five_days_ago {
                continue;
            }
            if trade.action != Action::Sell {
                continue;
            }
            let sell_date = trade.timestamp.date_naive();
            let bought_same_day = self.trade_history.iter().any(|t| {
                t.ticker == trade.ticker
                    && t.action == Action::Buy
                    && t.timestamp.date_naive() == sell_date
                    && t.timestamp < trade.timestamp
            });
            if bought_same_day {
                day_trade_count += 1;
            }
        }
        day_trade_count
    }

    fn check_daily_loss_limit(&self) -> Result<(), RuleViolation> {
        if self.daily_pnl < Decimal::ZERO
            && self.daily_pnl.abs() >= self.config.max_daily_loss_dollars
        {
            return Err(RuleViolation::DailyLossLimitReached);
        }
        Ok(())
    }

    fn check_daily_trade_count(&self) -> Result<(), RuleViolation> {
        let today = Utc::now().date_naive();
        let trades_today = self
            .trade_history
            .iter()
            .filter(|t| t.timestamp.date_naive() == today)
            .count() as u32;
        if trades_today >= self.config.max_daily_trades {
            return Err(RuleViolation::DailyTradeCountExceeded);
        }
        Ok(())
    }

    pub fn record_trade(&mut self, record: TradeRecord) {
        self.trade_history.push(record);
    }

    pub fn update_daily_pnl(&mut self, pnl: Decimal) {
        self.daily_pnl = pnl;
    }

    pub fn get_current_constraints(&self, _portfolio: &Portfolio) -> TradingConstraints {
        let today = Utc::now().date_naive();
        let trades_today = self
            .trade_history
            .iter()
            .filter(|t| t.timestamp.date_naive() == today)
            .count() as u32;
        let day_trades_used = self.count_day_trades_rolling_5_days();

        TradingConstraints {
            max_position_dollars: self.config.max_position_size_dollars,
            max_position_pct: self.config.max_position_pct,
            day_trades_remaining: self.config.day_trade_limit.saturating_sub(day_trades_used),
            daily_loss_remaining: self.config.max_daily_loss_dollars - self.daily_pnl.abs(),
            trades_remaining_today: self.config.max_daily_trades.saturating_sub(trades_today),
            allowed_tickers: self.config.allowed_tickers.clone(),
        }
    }

    pub fn is_dry_run(&self) -> bool {
        self.config.dry_run
    }

    pub fn load_history(&mut self, history: Vec<TradeRecord>, daily_pnl: Decimal) {
        self.trade_history = history;
        self.daily_pnl = daily_pnl;
    }

    pub fn trade_history(&self) -> &[TradeRecord] {
        &self.trade_history
    }

    pub fn daily_pnl(&self) -> Decimal {
        self.daily_pnl
    }

    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.config.dry_run = dry_run;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TradingConfig;
    use crate::llm::response::LlmOrderType;
    use crate::schwab::models::Quote;
    use chrono::Duration as ChronoDuration;
    use std::str::FromStr;

    fn make_trading_config() -> TradingConfig {
        TradingConfig {
            max_position_size_dollars: Decimal::from_str("10000").unwrap(),
            max_position_pct: Decimal::from_str("20").unwrap(),
            max_daily_trades: 10,
            max_daily_loss_dollars: Decimal::from_str("500").unwrap(),
            day_trade_limit: 3,
            allowed_tickers: None,
            blocked_tickers: vec!["GME".into(), "AMC".into(), "BBBY".into()],
            dry_run: true,
            watchlist: vec!["AAPL".into(), "MSFT".into()],
        }
    }

    fn make_portfolio(cash: Decimal, total: Decimal) -> Portfolio {
        Portfolio {
            cash_available: cash,
            positions: vec![],
            total_account_value: total,
        }
    }

    fn make_quote(symbol: &str, ask: Decimal) -> Quote {
        Quote {
            symbol: symbol.into(),
            bid_price: ask - Decimal::from_str("0.01").unwrap(),
            ask_price: ask,
            last_price: ask,
            total_volume: 1_000_000,
            high_price: ask + Decimal::from_str("1").unwrap(),
            low_price: ask - Decimal::from_str("1").unwrap(),
            open_price: ask - Decimal::from_str("0.50").unwrap(),
            close_price: ask - Decimal::from_str("0.25").unwrap(),
            quote_time: 0,
        }
    }

    fn make_buy_decision(ticker: &str, qty: u32) -> LlmDecision {
        LlmDecision {
            action: Action::Buy,
            ticker: Some(ticker.into()),
            quantity: Some(qty),
            order_type: LlmOrderType::Market,
            limit_price: None,
            reasoning: "Test buy".into(),
        }
    }

    fn make_sell_decision(ticker: &str, qty: u32) -> LlmDecision {
        LlmDecision {
            action: Action::Sell,
            ticker: Some(ticker.into()),
            quantity: Some(qty),
            order_type: LlmOrderType::Market,
            limit_price: None,
            reasoning: "Test sell".into(),
        }
    }

    #[test]
    fn blocked_ticker_rejected() {
        let rules = TradingRules::new(&make_trading_config());
        let decision = make_buy_decision("GME", 10);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("GME", Decimal::from_str("20").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::TickerNotAllowed(_)));
    }

    #[test]
    fn blocked_ticker_case_insensitive() {
        let rules = TradingRules::new(&make_trading_config());
        let decision = make_buy_decision("gme", 10);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("gme", Decimal::from_str("20").unwrap());

        assert!(rules.validate_trade(&decision, &portfolio, &quote).is_err());
    }

    #[test]
    fn allowed_ticker_whitelist() {
        let mut config = make_trading_config();
        config.allowed_tickers = Some(vec!["AAPL".into(), "MSFT".into()]);
        let rules = TradingRules::new(&config);

        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );

        // Allowed ticker passes
        let decision = make_buy_decision("AAPL", 10);
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());
        assert!(rules.validate_trade(&decision, &portfolio, &quote).is_ok());

        // Non-whitelisted ticker fails
        let decision = make_buy_decision("NVDA", 10);
        let quote = make_quote("NVDA", Decimal::from_str("500").unwrap());
        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::TickerNotAllowed(_)));
    }

    #[test]
    fn position_size_limit() {
        let rules = TradingRules::new(&make_trading_config());
        // 100 shares * $150 = $15,000 > $10,000 max
        let decision = make_buy_decision("AAPL", 100);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::PositionTooLarge { .. }));
    }

    #[test]
    fn position_percentage_limit() {
        let mut config = make_trading_config();
        config.max_position_size_dollars = Decimal::from_str("100000").unwrap(); // raise absolute limit
        config.max_position_pct = Decimal::from_str("10").unwrap();
        let rules = TradingRules::new(&config);

        // 100 shares * $150 = $15,000 = 15% of $100,000 > 10% max
        let decision = make_buy_decision("AAPL", 100);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(
            err,
            RuleViolation::PositionPercentageTooHigh { .. }
        ));
    }

    #[test]
    fn insufficient_cash() {
        let rules = TradingRules::new(&make_trading_config());
        // 50 shares * $150 = $7,500 > $5,000 cash
        let decision = make_buy_decision("AAPL", 50);
        let portfolio = make_portfolio(
            Decimal::from_str("5000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::InsufficientCash { .. }));
    }

    #[test]
    fn valid_buy_passes() {
        let rules = TradingRules::new(&make_trading_config());
        // 10 shares * $150 = $1,500 — well under all limits
        let decision = make_buy_decision("AAPL", 10);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        assert!(rules.validate_trade(&decision, &portfolio, &quote).is_ok());
    }

    #[test]
    fn sell_skips_position_size_check() {
        let rules = TradingRules::new(&make_trading_config());
        // Selling doesn't check position size / cash
        let decision = make_sell_decision("AAPL", 1000);
        let portfolio = make_portfolio(
            Decimal::from_str("100").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        assert!(rules.validate_trade(&decision, &portfolio, &quote).is_ok());
    }

    #[test]
    fn daily_loss_limit() {
        let mut rules = TradingRules::new(&make_trading_config());
        rules.update_daily_pnl(Decimal::from_str("-500").unwrap()); // at max loss

        let decision = make_buy_decision("AAPL", 1);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::DailyLossLimitReached));
    }

    #[test]
    fn daily_trade_count_limit() {
        let mut rules = TradingRules::new(&make_trading_config());
        let now = Utc::now();

        // Record 10 trades today (the max)
        for i in 0..10 {
            rules.record_trade(TradeRecord {
                timestamp: now,
                ticker: format!("SYM{i}"),
                action: Action::Buy,
                quantity: 1,
                price: Decimal::from_str("100").unwrap(),
                order_id: format!("ord-{i}"),
            });
        }

        let decision = make_buy_decision("AAPL", 1);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("150").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::DailyTradeCountExceeded));
    }

    #[test]
    fn day_trade_detection() {
        let mut config = make_trading_config();
        config.day_trade_limit = 1; // only allow 1 day trade
        let mut rules = TradingRules::new(&config);
        let now = Utc::now();

        // Record a buy + sell of AAPL today (one day trade used)
        rules.record_trade(TradeRecord {
            timestamp: now - ChronoDuration::minutes(30),
            ticker: "AAPL".into(),
            action: Action::Buy,
            quantity: 10,
            price: Decimal::from_str("150").unwrap(),
            order_id: "buy-1".into(),
        });
        rules.record_trade(TradeRecord {
            timestamp: now - ChronoDuration::minutes(15),
            ticker: "AAPL".into(),
            action: Action::Sell,
            quantity: 10,
            price: Decimal::from_str("155").unwrap(),
            order_id: "sell-1".into(),
        });

        // Now try to sell MSFT that was also bought today — would be 2nd day trade
        rules.record_trade(TradeRecord {
            timestamp: now - ChronoDuration::minutes(10),
            ticker: "MSFT".into(),
            action: Action::Buy,
            quantity: 5,
            price: Decimal::from_str("300").unwrap(),
            order_id: "buy-2".into(),
        });

        let decision = make_sell_decision("MSFT", 5);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("MSFT", Decimal::from_str("305").unwrap());

        let err = rules
            .validate_trade(&decision, &portfolio, &quote)
            .unwrap_err();
        assert!(matches!(err, RuleViolation::DayTradeLimitExceeded));
    }

    #[test]
    fn sell_not_bought_today_ok() {
        let mut rules = TradingRules::new(&make_trading_config());
        // Buy was yesterday, sell today — not a day trade
        let yesterday = Utc::now() - ChronoDuration::days(1);
        rules.record_trade(TradeRecord {
            timestamp: yesterday,
            ticker: "AAPL".into(),
            action: Action::Buy,
            quantity: 10,
            price: Decimal::from_str("150").unwrap(),
            order_id: "buy-1".into(),
        });

        let decision = make_sell_decision("AAPL", 10);
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let quote = make_quote("AAPL", Decimal::from_str("155").unwrap());

        assert!(rules.validate_trade(&decision, &portfolio, &quote).is_ok());
    }

    #[test]
    fn constraints_reflect_state() {
        let mut rules = TradingRules::new(&make_trading_config());
        let portfolio = make_portfolio(
            Decimal::from_str("50000").unwrap(),
            Decimal::from_str("100000").unwrap(),
        );
        let now = Utc::now();

        // Record 3 trades today
        for i in 0..3 {
            rules.record_trade(TradeRecord {
                timestamp: now,
                ticker: format!("SYM{i}"),
                action: Action::Buy,
                quantity: 1,
                price: Decimal::from_str("100").unwrap(),
                order_id: format!("ord-{i}"),
            });
        }
        rules.update_daily_pnl(Decimal::from_str("-200").unwrap());

        let constraints = rules.get_current_constraints(&portfolio);
        assert_eq!(constraints.trades_remaining_today, 7); // 10 - 3
        assert_eq!(
            constraints.daily_loss_remaining,
            Decimal::from_str("300").unwrap()
        ); // 500 - 200
        assert_eq!(
            constraints.max_position_dollars,
            Decimal::from_str("10000").unwrap()
        );
    }

    #[test]
    fn dry_run_flag() {
        let config = make_trading_config();
        let rules = TradingRules::new(&config);
        assert!(rules.is_dry_run());
    }
}
