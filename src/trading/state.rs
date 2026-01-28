use crate::trading::rules::TradeRecord;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct TradingState {
    pub trade_history: Vec<TradeRecord>,
    pub daily_pnl: Decimal,
    pub last_reset_date: NaiveDate,
}

impl TradingState {
    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        let today = Utc::now().date_naive();

        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            let mut state: TradingState = serde_json::from_str(&data)?;

            // Reset daily P&L if it's a new day
            if state.last_reset_date != today {
                state.daily_pnl = Decimal::ZERO;
                state.last_reset_date = today;
            }

            // Prune trades older than 30 days
            let cutoff = Utc::now() - chrono::Duration::days(30);
            state.trade_history.retain(|t| t.timestamp >= cutoff);

            Ok(state)
        } else {
            Ok(Self {
                trade_history: Vec::new(),
                daily_pnl: Decimal::ZERO,
                last_reset_date: today,
            })
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}
