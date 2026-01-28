use crate::error::{BotError, Result};
use crate::llm::response::{Action, LlmDecision, LlmOrderType};
use crate::schwab::client::SchwabClient;
use crate::schwab::orders;
use crate::trading::decision::ExecutionResult;
use crate::trading::portfolio::Portfolio;
use crate::trading::rules::{TradeRecord, TradingRules};
use chrono::Utc;
use std::sync::Arc;

pub struct TradeExecutor {
    schwab: Arc<SchwabClient>,
    pub rules: TradingRules,
    account_id: String,
}

impl TradeExecutor {
    pub fn new(schwab: Arc<SchwabClient>, rules: TradingRules, account_id: String) -> Self {
        Self {
            schwab,
            rules,
            account_id,
        }
    }

    pub async fn execute(
        &mut self,
        decision: LlmDecision,
        portfolio: &Portfolio,
    ) -> Result<ExecutionResult> {
        if decision.action == Action::Hold {
            return Ok(ExecutionResult::Held {
                reason: decision.reasoning,
            });
        }

        let ticker = decision
            .ticker
            .as_ref()
            .ok_or_else(|| BotError::Other("Missing ticker for BUY/SELL".into()))?;

        // Get fresh quote
        let quote = self.schwab.get_quote(ticker).await?;

        // Validate against rules
        self.rules.validate_trade(&decision, portfolio, &quote)?;

        let quantity = decision.quantity.unwrap_or(0);

        // Dry run mode
        if self.rules.is_dry_run() {
            tracing::info!(
                ticker = ticker,
                action = ?decision.action,
                quantity = quantity,
                price = %quote.last_price,
                "DRY RUN - would have placed order"
            );
            self.rules.record_trade(TradeRecord {
                timestamp: Utc::now(),
                ticker: ticker.clone(),
                action: decision.action,
                quantity,
                price: quote.last_price,
                order_id: "dry-run".to_string(),
            });
            return Ok(ExecutionResult::DryRun {
                ticker: ticker.clone(),
                quantity,
                price: quote.last_price,
                reason: decision.reasoning,
            });
        }

        // Build order
        let order = match (&decision.action, &decision.order_type) {
            (Action::Buy, LlmOrderType::Market) => orders::build_market_buy(ticker, quantity),
            (Action::Sell, LlmOrderType::Market) => orders::build_market_sell(ticker, quantity),
            (Action::Buy, LlmOrderType::Limit) => {
                let price = decision
                    .limit_price
                    .ok_or_else(|| BotError::Other("Limit order missing price".into()))?;
                orders::build_limit_buy(ticker, quantity, price)
            }
            (Action::Sell, LlmOrderType::Limit) => {
                let price = decision
                    .limit_price
                    .ok_or_else(|| BotError::Other("Limit order missing price".into()))?;
                orders::build_limit_sell(ticker, quantity, price)
            }
            (Action::Hold, _) => unreachable!(),
        };

        // Submit order
        let response = self.schwab.place_order(&self.account_id, &order).await?;

        // Record trade
        self.rules.record_trade(TradeRecord {
            timestamp: Utc::now(),
            ticker: ticker.clone(),
            action: decision.action,
            quantity,
            price: quote.last_price,
            order_id: response.order_id.clone(),
        });

        Ok(ExecutionResult::Executed {
            order_id: response.order_id,
            ticker: ticker.clone(),
            quantity,
            price: quote.last_price,
        })
    }
}
