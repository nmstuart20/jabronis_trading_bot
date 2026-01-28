use crate::schwab::models::Position;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct Portfolio {
    pub cash_available: Decimal,
    pub positions: Vec<Position>,
    pub total_account_value: Decimal,
}

impl Portfolio {
    pub fn total_value(&self) -> Decimal {
        self.total_account_value
    }

    pub fn position_for(&self, symbol: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.symbol == symbol)
    }
}
