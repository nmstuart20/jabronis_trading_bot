use crate::llm::sanitizer::MarketContext;
use crate::trading::decision::TradingConstraints;
use crate::trading::portfolio::Portfolio;
use rust_decimal::Decimal;

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

        // Build a cash warning if cash is low relative to positions held
        let cash_warning = Self::build_cash_warning(portfolio);

        format!(
            r#"You are a trading assistant. Analyze the market data and decide on a trading action.
{mode_instructions}
## Current Portfolio
Cash Available: ${cash}
Positions:
{positions}
{cash_warning}
## Constraints (STRICTLY ENFORCED - you cannot override these)
- Max position size: ${max_pos} or {max_pct}% of portfolio
- Remaining day trades this week: {day_trades_left}
- Max daily loss remaining: ${loss_remaining}
- Allowed to trade: {allowed}
- **A BUY order WILL BE REJECTED if its cost exceeds Cash Available (${cash}).** If you want to BUY but lack cash, you MUST use the two-action plan format to SELL a position first.

## Current Market Data
{market_context}

## Your Task
Analyze the data and recommend an action. You must respond with ONLY a JSON object.

IMPORTANT RULES:
1. You CANNOT buy shares costing more than your Cash Available (${cash}). Any such order will fail.
2. If you want to BUY but do not have enough cash, you MUST sell an existing position first using the two-action plan format below.
3. When using the two-action plan, the SELL must free up enough cash to cover the BUY.
4. If you have no positions to sell and no cash, you MUST respond with HOLD.

**Single action format** (for HOLD, a standalone BUY with enough cash, or a standalone SELL):
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

**Two-action plan format** (SELL then BUY when cash is insufficient):
```json
{{
  "actions": [
    {{
      "action": "SELL",
      "ticker": "SYMBOL",
      "quantity": number,
      "order_type": "MARKET" | "LIMIT",
      "limit_price": number | null,
      "reasoning": "Brief explanation (max 200 chars)"
    }},
    {{
      "action": "BUY",
      "ticker": "SYMBOL",
      "quantity": number,
      "order_type": "MARKET" | "LIMIT",
      "limit_price": number | null,
      "reasoning": "Brief explanation (max 200 chars)"
    }}
  ]
}}
```

If no good opportunity exists, respond with action "HOLD".
Do not include any text outside the JSON object."#,
            cash = portfolio.cash_available,
            positions = positions_str,
            cash_warning = cash_warning,
            max_pos = constraints.max_position_dollars,
            max_pct = constraints.max_position_pct,
            day_trades_left = constraints.day_trades_remaining,
            loss_remaining = constraints.daily_loss_remaining,
            allowed = allowed,
            market_context = market_context,
            mode_instructions = mode_instructions,
        )
    }

    /// Generate a prominent warning when cash is too low to make meaningful purchases.
    fn build_cash_warning(portfolio: &Portfolio) -> String {
        let has_positions = !portfolio.positions.is_empty();
        let low_cash_threshold = Decimal::from(100);

        if portfolio.cash_available < low_cash_threshold && has_positions {
            format!(
                "\n⚠ LOW CASH WARNING: You only have ${} in cash. To BUY anything, you MUST SELL an existing position first using the two-action plan format.\n",
                portfolio.cash_available
            )
        } else if portfolio.cash_available < low_cash_threshold && !has_positions {
            format!(
                "\n⚠ LOW CASH WARNING: You only have ${} in cash and no positions to sell. You MUST respond with HOLD.\n",
                portfolio.cash_available
            )
        } else {
            String::new()
        }
    }
}
