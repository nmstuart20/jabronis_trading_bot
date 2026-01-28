use crate::error::Result;
use crate::schwab::client::SchwabClient;
use crate::schwab::models::{Candle, FrequencyType, PeriodType};
use std::collections::HashMap;
use std::sync::Arc;

pub struct HistoricalDataService {
    schwab: Arc<SchwabClient>,
}

impl HistoricalDataService {
    pub fn new(schwab: Arc<SchwabClient>) -> Self {
        Self { schwab }
    }

    pub async fn get_daily_bars(&self, symbol: &str, days: u32) -> Result<Vec<Candle>> {
        let history = self
            .schwab
            .get_price_history(symbol, PeriodType::Day, days, FrequencyType::Daily, 1)
            .await?;
        Ok(history.candles)
    }

    pub async fn get_intraday_bars(&self, symbol: &str, minutes: u32) -> Result<Vec<Candle>> {
        let history = self
            .schwab
            .get_price_history(symbol, PeriodType::Day, 1, FrequencyType::Minute, minutes)
            .await?;
        Ok(history.candles)
    }

    pub async fn get_recent_bars(
        &self,
        symbols: &[&str],
    ) -> Result<HashMap<String, Vec<Candle>>> {
        let mut result = HashMap::new();
        for &sym in symbols {
            match self.get_daily_bars(sym, 10).await {
                Ok(bars) => {
                    result.insert(sym.to_string(), bars);
                }
                Err(e) => {
                    tracing::warn!(symbol = sym, error = %e, "Failed to fetch historical bars");
                }
            }
        }
        Ok(result)
    }
}
