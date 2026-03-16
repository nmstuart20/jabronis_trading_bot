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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn make_portfolio(cash: Decimal, total: Decimal) -> Portfolio {
        Portfolio {
            cash_available: cash,
            positions: vec![],
            total_account_value: total,
        }
    }

    #[test]
    fn total_value() {
        let p = make_portfolio(
            Decimal::from_str("10000").unwrap(),
            Decimal::from_str("50000").unwrap(),
        );
        assert_eq!(p.total_value(), Decimal::from_str("50000").unwrap());
    }

    #[test]
    fn position_for_found() {
        let p = Portfolio {
            cash_available: Decimal::from_str("10000").unwrap(),
            positions: vec![Position {
                symbol: "AAPL".into(),
                quantity: Decimal::from_str("10").unwrap(),
                average_price: Decimal::from_str("150").unwrap(),
                current_value: Decimal::from_str("1550").unwrap(),
                unrealized_pnl: Decimal::from_str("50").unwrap(),
            }],
            total_account_value: Decimal::from_str("11550").unwrap(),
        };
        assert!(p.position_for("AAPL").is_some());
        assert_eq!(
            p.position_for("AAPL").unwrap().quantity,
            Decimal::from_str("10").unwrap()
        );
    }

    #[test]
    fn position_for_not_found() {
        let p = make_portfolio(
            Decimal::from_str("10000").unwrap(),
            Decimal::from_str("10000").unwrap(),
        );
        assert!(p.position_for("AAPL").is_none());
    }
}
