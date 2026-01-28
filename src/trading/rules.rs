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
                t.ticker == *ticker
                    && t.action == Action::Buy
                    && t.timestamp.date_naive() == today
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
