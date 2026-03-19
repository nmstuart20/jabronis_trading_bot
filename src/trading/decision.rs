use crate::schwab::models::PreviewOrderResponse;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct TradingConstraints {
    pub max_position_dollars: Decimal,
    pub max_position_pct: Decimal,
    pub day_trades_remaining: u32,
    pub daily_loss_remaining: Decimal,
    pub trades_remaining_today: u32,
    pub allowed_tickers: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum ExecutionResult {
    Held {
        reason: String,
    },
    Executed {
        order_id: String,
        ticker: String,
        quantity: u32,
        price: Decimal,
    },
    DryRun {
        ticker: String,
        quantity: u32,
        price: Decimal,
        reason: String,
        preview: Box<Option<PreviewOrderResponse>>,
    },
}
