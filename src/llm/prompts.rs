use crate::llm::sanitizer::MarketContext;
use crate::trading::decision::TradingConstraints;
use crate::trading::portfolio::Portfolio;
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_trading_prompt(
        context: &MarketContext,
        portfolio: &Portfolio,
        constraints: &TradingConstraints,
        mode: &str,
    ) -> String {
        let positions_str = if portfolio.positions.is_empty() {
            "None".to_string()
        } else {
            portfolio
                .positions
                .iter()
                .map(|p| {
                    format!(
                        "  {} : {} shares @ ${} (P&L: ${})",
                        p.symbol, p.quantity, p.average_price, p.unrealized_pnl
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let allowed = constraints
            .allowed_tickers
            .as_ref()
            .map(|t| t.join(", "))
            .unwrap_or_else(|| "any".to_string());

        let market_context =
            serde_json::to_string_pretty(context).unwrap_or_else(|_| "{}".to_string());

        let mode_instructions = match mode {
            "open" => "\n## Session Mode: MARKET OPEN\nFocus on opening momentum and gap analysis. Look for early trends and volume confirmations.\n",
            "midday" => "\n## Session Mode: MIDDAY\nFocus on trend continuation or reversal signals. Consider intraday support/resistance levels.\n",
            "preclose" => "\n## Session Mode: PRE-CLOSE\nMarket closing soon. Avoid opening new large positions. Consider closing intraday positions to avoid overnight risk.\n",
            "manual" => "\n## Session Mode: MANUAL\nThis is a manually triggered analysis. Provide your best assessment regardless of time of day.\n",
            _ => "",
        };

        format!(
            r#"You are a trading assistant. Analyze the market data and decide on a trading action.
{mode_instructions}
## Current Portfolio
Cash Available: ${cash}
Positions:
{positions}

## Constraints (STRICTLY ENFORCED - you cannot override these)
- Max position size: ${max_pos} or {max_pct}% of portfolio
- Remaining day trades this week: {day_trades_left}
- Max daily loss remaining: ${loss_remaining}
- Allowed to trade: {allowed}

## Current Market Data
{market_context}

## Your Task
Analyze the data and recommend ONE action. You must respond with ONLY a JSON object in this exact format:

```json
{{
  "action": "BUY" | "SELL" | "HOLD",
  "ticker": "SYMBOL" | null,
  "quantity": number | null,
  "order_type": "MARKET" | "LIMIT",
  "limit_price": number | null,
  "reasoning": "Brief explanation (max 200 chars)"
}}
```

If no good opportunity exists, respond with action "HOLD".
Do not include any text outside the JSON object."#,
            cash = portfolio.cash_available,
            positions = positions_str,
            max_pos = constraints.max_position_dollars,
            max_pct = constraints.max_position_pct,
            day_trades_left = constraints.day_trades_remaining,
            loss_remaining = constraints.daily_loss_remaining,
            allowed = allowed,
            market_context = market_context,
            mode_instructions = mode_instructions,
        )
    }
}
