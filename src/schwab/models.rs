use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde(default)]
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

/// Internal position representation used throughout the bot.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub current_value: Decimal,
    pub unrealized_pnl: Decimal,
}

/// Raw position as returned by the Schwab API, nested inside securitiesAccount.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchwabPosition {
    pub instrument: Instrument,
    #[serde(default)]
    pub long_quantity: Decimal,
    #[serde(default)]
    pub short_quantity: Decimal,
    #[serde(default)]
    pub average_price: Decimal,
    #[serde(default)]
    pub market_value: Decimal,
    #[serde(default)]
    pub current_day_profit_loss: Decimal,
}

impl SchwabPosition {
    pub fn into_position(self) -> Position {
        let quantity = self.long_quantity - self.short_quantity;
        Position {
            symbol: self.instrument.symbol,
            quantity,
            average_price: self.average_price,
            current_value: self.market_value,
            unrealized_pnl: self.current_day_profit_loss,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountNumberHash {
    pub account_number: String,
    pub hash_value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub securities_account: Option<SecuritiesAccount>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecuritiesAccount {
    pub account_number: Option<String>,
    #[serde(default)]
    pub current_balances: Option<AccountBalances>,
    #[serde(default)]
    pub positions: Vec<SchwabPosition>,
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
    Index,
    CashEquivalent,
    FixedIncome,
    Currency,
    CollectiveInvestment,
    #[serde(other)]
    Unknown,
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
