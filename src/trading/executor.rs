use crate::error::{BotError, Result};
use crate::llm::response::{Action, LlmDecision, LlmOrderType, LlmPlan};
use crate::schwab::client::SchwabClient;
use crate::schwab::models::PreviewOrderResponse;
use crate::schwab::orders;
use crate::trading::decision::ExecutionResult;
use crate::trading::portfolio::Portfolio;
use crate::trading::rules::{TradeRecord, TradingRules};
use chrono::Utc;
use rust_decimal::Decimal;
use std::sync::Arc;

pub struct TradeExecutor {
    schwab: Arc<SchwabClient>,
    pub rules: TradingRules,
}

impl TradeExecutor {
    pub fn new(schwab: Arc<SchwabClient>, rules: TradingRules) -> Self {
        Self { schwab, rules }
    }

    /// Execute a plan of 1-2 actions sequentially.
    /// For a SELL+BUY plan, the sell proceeds are added to a shadow copy of the portfolio
    /// so the BUY's cash check accounts for the freed-up funds.
    pub async fn execute_plan(
        &mut self,
        plan: LlmPlan,
        portfolio: &Portfolio,
    ) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();
        let mut shadow_portfolio = portfolio.clone();

        for decision in plan.actions {
            let result = self.execute(decision, &shadow_portfolio).await?;

            // After a successful SELL, add estimated proceeds to shadow portfolio cash
            // so the next action (BUY) can pass the cash check.
            if let ExecutionResult::Executed {
                quantity, price, ..
            }
            | ExecutionResult::DryRun {
                quantity, price, ..
            } = &result
            {
                // For sells, add proceeds to available cash
                let trade_value = Decimal::from(*quantity) * price;
                // Check if this was a sell by looking at the result type context
                // We recorded the trade, so check the last record
                if let Some(last) = self.rules.trade_history().last() {
                    if last.action == Action::Sell {
                        shadow_portfolio.cash_available += trade_value;
                    }
                }
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Preview an order through Schwab's API without placing it.
    /// Validates rules locally, builds the order, and sends it to Schwab's
    /// preview endpoint to check for rejects, warnings, and estimated fees.
    pub async fn preview(
        &self,
        decision: &LlmDecision,
        portfolio: &Portfolio,
    ) -> Result<PreviewOrderResponse> {
        if decision.action == Action::Hold {
            return Err(BotError::Other("Cannot preview a HOLD action".into()));
        }

        let ticker = decision
            .ticker
            .as_ref()
            .ok_or_else(|| BotError::Other("Missing ticker for BUY/SELL".into()))?;

        let quote = self.schwab.get_quote(ticker).await?;
        self.rules.validate_trade(decision, portfolio, &quote)?;

        let quantity = decision.quantity.unwrap_or(0);
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

        self.schwab.preview_order(&order).await
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
        let response = self.schwab.place_order(&order).await?;

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
