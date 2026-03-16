use crate::schwab::models::*;

pub fn build_market_buy(symbol: &str, quantity: u32) -> Order {
    Order {
        order_type: OrderType::Market,
        session: Session::Normal,
        duration: Duration::Day,
        order_strategy_type: OrderStrategyType::Single,
        price: None,
        order_leg_collection: vec![OrderLeg {
            instruction: Instruction::Buy,
            quantity,
            instrument: Instrument {
                symbol: symbol.to_string(),
                asset_type: AssetType::Equity,
            },
        }],
    }
}

pub fn build_market_sell(symbol: &str, quantity: u32) -> Order {
    Order {
        order_type: OrderType::Market,
        session: Session::Normal,
        duration: Duration::Day,
        order_strategy_type: OrderStrategyType::Single,
        price: None,
        order_leg_collection: vec![OrderLeg {
            instruction: Instruction::Sell,
            quantity,
            instrument: Instrument {
                symbol: symbol.to_string(),
                asset_type: AssetType::Equity,
            },
        }],
    }
}

pub fn build_limit_buy(symbol: &str, quantity: u32, price: rust_decimal::Decimal) -> Order {
    Order {
        order_type: OrderType::Limit,
        session: Session::Normal,
        duration: Duration::Day,
        order_strategy_type: OrderStrategyType::Single,
        price: Some(price.to_string()),
        order_leg_collection: vec![OrderLeg {
            instruction: Instruction::Buy,
            quantity,
            instrument: Instrument {
                symbol: symbol.to_string(),
                asset_type: AssetType::Equity,
            },
        }],
    }
}

pub fn build_limit_sell(symbol: &str, quantity: u32, price: rust_decimal::Decimal) -> Order {
    Order {
        order_type: OrderType::Limit,
        session: Session::Normal,
        duration: Duration::Day,
        order_strategy_type: OrderStrategyType::Single,
        price: Some(price.to_string()),
        order_leg_collection: vec![OrderLeg {
            instruction: Instruction::Sell,
            quantity,
            instrument: Instrument {
                symbol: symbol.to_string(),
                asset_type: AssetType::Equity,
            },
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn market_buy_order() {
        let order = build_market_buy("AAPL", 10);
        assert!(matches!(order.order_type, OrderType::Market));
        assert!(order.price.is_none());
        assert_eq!(order.order_leg_collection.len(), 1);
        let leg = &order.order_leg_collection[0];
        assert!(matches!(leg.instruction, Instruction::Buy));
        assert_eq!(leg.quantity, 10);
        assert_eq!(leg.instrument.symbol, "AAPL");
        assert!(matches!(leg.instrument.asset_type, AssetType::Equity));
    }

    #[test]
    fn market_sell_order() {
        let order = build_market_sell("MSFT", 5);
        assert!(matches!(order.order_type, OrderType::Market));
        assert!(order.price.is_none());
        assert!(matches!(
            order.order_leg_collection[0].instruction,
            Instruction::Sell
        ));
        assert_eq!(order.order_leg_collection[0].quantity, 5);
    }

    #[test]
    fn limit_buy_order() {
        let order = build_limit_buy("NVDA", 20, Decimal::from_str("500.50").unwrap());
        assert!(matches!(order.order_type, OrderType::Limit));
        assert_eq!(order.price, Some("500.50".to_string()));
        assert!(matches!(
            order.order_leg_collection[0].instruction,
            Instruction::Buy
        ));
        assert_eq!(order.order_leg_collection[0].quantity, 20);
    }

    #[test]
    fn limit_sell_order() {
        let order = build_limit_sell("GOOGL", 15, Decimal::from_str("175.25").unwrap());
        assert!(matches!(order.order_type, OrderType::Limit));
        assert_eq!(order.price, Some("175.25".to_string()));
        assert!(matches!(
            order.order_leg_collection[0].instruction,
            Instruction::Sell
        ));
    }

    #[test]
    fn all_orders_are_day_single_normal() {
        for order in [
            build_market_buy("X", 1),
            build_market_sell("X", 1),
            build_limit_buy("X", 1, Decimal::from_str("100").unwrap()),
            build_limit_sell("X", 1, Decimal::from_str("100").unwrap()),
        ] {
            assert!(matches!(order.session, Session::Normal));
            assert!(matches!(order.duration, Duration::Day));
            assert!(matches!(
                order.order_strategy_type,
                OrderStrategyType::Single
            ));
        }
    }
}
