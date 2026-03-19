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
pub struct PreviewOrderResponse {
    #[serde(default)]
    pub order_id: i64,
    #[serde(default)]
    pub order_strategy: Option<PreviewOrderStrategy>,
    #[serde(default)]
    pub order_validation_result: Option<OrderValidationResult>,
    #[serde(default)]
    pub commission_and_fee: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOrderStrategy {
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub quantity: Option<f64>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub order_legs: Vec<PreviewOrderLeg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOrderLeg {
    #[serde(default)]
    pub ask_price: Option<f64>,
    #[serde(default)]
    pub bid_price: Option<f64>,
    #[serde(default)]
    pub last_price: Option<f64>,
    #[serde(default)]
    pub projected_commission: Option<f64>,
    #[serde(default)]
    pub quantity: Option<f64>,
    #[serde(default)]
    pub final_symbol: Option<String>,
    #[serde(default)]
    pub instruction: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderValidationResult {
    #[serde(default)]
    pub alerts: Vec<ValidationMessage>,
    #[serde(default)]
    pub accepts: Vec<ValidationMessage>,
    #[serde(default)]
    pub rejects: Vec<ValidationMessage>,
    #[serde(default)]
    pub reviews: Vec<ValidationMessage>,
    #[serde(default)]
    pub warns: Vec<ValidationMessage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationMessage {
    #[serde(default)]
    pub validation_rule_name: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub activity_message: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::response::Action;
    use crate::schwab::orders;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn order_type_serializes_screaming_snake() {
        let order = orders::build_market_buy("AAPL", 1);
        let json = serde_json::to_string(&order).unwrap();
        assert!(json.contains("\"MARKET\""));
        assert!(json.contains("\"BUY\""));
        assert!(json.contains("\"NORMAL\""));
        assert!(json.contains("\"DAY\""));
        assert!(json.contains("\"SINGLE\""));
    }

    #[test]
    fn limit_order_includes_price() {
        let order = orders::build_limit_buy("AAPL", 1, Decimal::from_str("150").unwrap());
        let json = serde_json::to_string(&order).unwrap();
        assert!(json.contains("\"LIMIT\""));
        assert!(json.contains("\"price\""));
    }

    #[test]
    fn market_order_omits_price() {
        let order = orders::build_market_buy("AAPL", 1);
        let json = serde_json::to_string(&order).unwrap();
        // price field has skip_serializing_if = "Option::is_none"
        assert!(!json.contains("\"price\""));
    }

    #[test]
    fn quote_deserializes_camel_case() {
        let json = r#"{
            "symbol": "AAPL",
            "bidPrice": "149.99",
            "askPrice": "150.01",
            "lastPrice": "150.00",
            "totalVolume": 50000000,
            "highPrice": "152.00",
            "lowPrice": "148.00",
            "openPrice": "149.00",
            "closePrice": "148.50"
        }"#;
        let quote: Quote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.symbol, "AAPL");
        assert_eq!(quote.bid_price, Decimal::from_str("149.99").unwrap());
        assert_eq!(quote.total_volume, 50_000_000);
    }

    #[test]
    fn llm_action_deserializes_screaming() {
        let json = r#""BUY""#;
        let action: Action = serde_json::from_str(json).unwrap();
        assert_eq!(action, Action::Buy);
    }
}
