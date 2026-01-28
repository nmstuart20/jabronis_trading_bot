use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub symbol: String,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub total_volume: u64,
    pub high_price: Decimal,
    pub low_price: Decimal,
    pub open_price: Decimal,
    pub close_price: Decimal,
    #[serde(default)]
    pub quote_time: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub current_value: Decimal,
    pub unrealized_pnl: Decimal,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_id: String,
    #[serde(default)]
    pub current_balances: Option<AccountBalances>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalances {
    pub cash_available_for_trading: Decimal,
    pub liquidation_value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub order_type: OrderType,
    pub session: Session,
    pub duration: Duration,
    pub order_strategy_type: OrderStrategyType,
    pub order_leg_collection: Vec<OrderLeg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderLeg {
    pub instruction: Instruction,
    pub quantity: u32,
    pub instrument: Instrument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub symbol: String,
    pub asset_type: AssetType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Session {
    Normal,
    Am,
    Pm,
    Seamless,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Duration {
    Day,
    GoodTillCancel,
    FillOrKill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStrategyType {
    Single,
    Trigger,
    Oco,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Instruction {
    Buy,
    Sell,
    BuyToCover,
    SellShort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetType {
    Equity,
    Option,
    MutualFund,
    Etf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub order_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceHistory {
    pub symbol: String,
    pub candles: Vec<Candle>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub datetime: i64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PeriodType {
    Day,
    Month,
    Year,
    Ytd,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FrequencyType {
    Minute,
    Daily,
    Weekly,
    Monthly,
}
