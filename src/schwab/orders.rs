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
