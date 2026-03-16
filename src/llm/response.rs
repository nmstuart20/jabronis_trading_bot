use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LlmDecision {
    pub action: Action,
    pub ticker: Option<String>,
    pub quantity: Option<u32>,
    pub order_type: LlmOrderType,
    pub limit_price: Option<Decimal>,
    pub reasoning: String,
}

/// A plan consisting of one or more actions to execute sequentially.
/// Used when the LLM recommends a SELL-then-BUY pair to free up cash.
#[derive(Debug)]
pub struct LlmPlan {
    pub actions: Vec<LlmDecision>,
}

/// Raw multi-action response format from the LLM.
#[derive(Debug, Deserialize)]
struct MultiActionResponse {
    actions: Vec<LlmDecision>,
}

#[derive(Debug, serde::Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LlmOrderType {
    Market,
    Limit,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("No JSON found in response")]
    NoJson,
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

pub struct ResponseParser;

impl ResponseParser {
    /// Parse a single-action response (backwards compatible).
    pub fn parse(response: &str) -> Result<LlmDecision, ParseError> {
        let json_str = Self::extract_json(response)?;
        let decision: LlmDecision =
            serde_json::from_str(json_str).map_err(|e| ParseError::InvalidJson(e.to_string()))?;
        Self::validate(&decision)?;
        Ok(decision)
    }

    /// Parse a response that may contain either a single action or a multi-action plan.
    /// Returns an `LlmPlan` with 1-2 actions.
    pub fn parse_plan(response: &str) -> Result<LlmPlan, ParseError> {
        let json_str = Self::extract_json(response)?;

        // Try multi-action format first: { "actions": [...] }
        if let Ok(multi) = serde_json::from_str::<MultiActionResponse>(json_str) {
            if multi.actions.is_empty() {
                return Err(ParseError::ValidationFailed(
                    "Actions array must not be empty".into(),
                ));
            }
            if multi.actions.len() > 2 {
                return Err(ParseError::ValidationFailed(
                    "Maximum 2 actions allowed per plan".into(),
                ));
            }
            // Validate ordering: if 2 actions, first must be SELL and second must be BUY
            if multi.actions.len() == 2 {
                if multi.actions[0].action != Action::Sell {
                    return Err(ParseError::ValidationFailed(
                        "First action in a two-action plan must be SELL".into(),
                    ));
                }
                if multi.actions[1].action != Action::Buy {
                    return Err(ParseError::ValidationFailed(
                        "Second action in a two-action plan must be BUY".into(),
                    ));
                }
            }
            for action in &multi.actions {
                Self::validate(action)?;
            }
            return Ok(LlmPlan {
                actions: multi.actions,
            });
        }

        // Fall back to single-action format
        let decision: LlmDecision =
            serde_json::from_str(json_str).map_err(|e| ParseError::InvalidJson(e.to_string()))?;
        Self::validate(&decision)?;
        Ok(LlmPlan {
            actions: vec![decision],
        })
    }

    fn extract_json(text: &str) -> Result<&str, ParseError> {
        // Try to find ```json ... ``` block
        if let Some(start) = text.find("```json") {
            let content = &text[start + 7..];
            if let Some(end) = content.find("```") {
                return Ok(content[..end].trim());
            }
        }
        // Try to find ``` ... ``` block
        if let Some(start) = text.find("```") {
            let content = &text[start + 3..];
            if let Some(end) = content.find("```") {
                return Ok(content[..end].trim());
            }
        }
        // Try raw JSON (find first { and last })
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if end > start {
                    return Ok(&text[start..=end]);
                }
            }
        }
        Err(ParseError::NoJson)
    }

    fn validate(decision: &LlmDecision) -> Result<(), ParseError> {
        match decision.action {
            Action::Buy | Action::Sell => {
                if decision.ticker.is_none() {
                    return Err(ParseError::ValidationFailed(
                        "BUY/SELL requires a ticker".into(),
                    ));
                }
                if decision.quantity.is_none() || decision.quantity == Some(0) {
                    return Err(ParseError::ValidationFailed(
                        "BUY/SELL requires a positive quantity".into(),
                    ));
                }
                if decision.order_type == LlmOrderType::Limit && decision.limit_price.is_none() {
                    return Err(ParseError::ValidationFailed(
                        "LIMIT order requires a limit_price".into(),
                    ));
                }
            }
            Action::Hold => {}
        }
        if decision.reasoning.is_empty() {
            return Err(ParseError::ValidationFailed(
                "Reasoning must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    mod response_parser {
        use super::*;

        #[test]
        fn parse_json_code_block() {
            let input = r#"Here is my decision:
```json
{
  "action": "HOLD",
  "ticker": null,
  "quantity": null,
  "order_type": "MARKET",
  "limit_price": null,
  "reasoning": "Market conditions uncertain"
}
```
"#;
            let decision = ResponseParser::parse(input).unwrap();
            assert_eq!(decision.action, Action::Hold);
            assert!(decision.ticker.is_none());
        }

        #[test]
        fn parse_generic_code_block() {
            let input = r#"```
{
  "action": "BUY",
  "ticker": "AAPL",
  "quantity": 10,
  "order_type": "MARKET",
  "limit_price": null,
  "reasoning": "Strong momentum"
}
```"#;
            let decision = ResponseParser::parse(input).unwrap();
            assert_eq!(decision.action, Action::Buy);
            assert_eq!(decision.ticker.as_deref(), Some("AAPL"));
            assert_eq!(decision.quantity, Some(10));
        }

        #[test]
        fn parse_raw_json() {
            let input = r#"I think we should buy. {"action": "BUY", "ticker": "MSFT", "quantity": 5, "order_type": "LIMIT", "limit_price": "350.00", "reasoning": "Good value"} That's my take."#;
            let decision = ResponseParser::parse(input).unwrap();
            assert_eq!(decision.action, Action::Buy);
            assert_eq!(decision.ticker.as_deref(), Some("MSFT"));
            assert_eq!(decision.order_type, LlmOrderType::Limit);
            assert_eq!(
                decision.limit_price,
                Some(Decimal::from_str("350.00").unwrap())
            );
        }

        #[test]
        fn parse_no_json() {
            let input = "I think we should hold for now and wait for better conditions.";
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::NoJson));
        }

        #[test]
        fn parse_invalid_json() {
            let input = r#"```json
{ "action": "BUY", "ticker": }
```"#;
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::InvalidJson(_)));
        }

        #[test]
        fn validate_buy_missing_ticker() {
            let input = r#"{"action": "BUY", "ticker": null, "quantity": 10, "order_type": "MARKET", "limit_price": null, "reasoning": "Buy something"}"#;
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn validate_buy_zero_quantity() {
            let input = r#"{"action": "BUY", "ticker": "AAPL", "quantity": 0, "order_type": "MARKET", "limit_price": null, "reasoning": "Buy zero"}"#;
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn validate_buy_missing_quantity() {
            let input = r#"{"action": "SELL", "ticker": "AAPL", "quantity": null, "order_type": "MARKET", "limit_price": null, "reasoning": "Sell some"}"#;
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn validate_limit_missing_price() {
            let input = r#"{"action": "BUY", "ticker": "AAPL", "quantity": 10, "order_type": "LIMIT", "limit_price": null, "reasoning": "Limit buy"}"#;
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn validate_empty_reasoning() {
            let input = r#"{"action": "HOLD", "ticker": null, "quantity": null, "order_type": "MARKET", "limit_price": null, "reasoning": ""}"#;
            let err = ResponseParser::parse(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn validate_hold_no_ticker_ok() {
            let input = r#"{"action": "HOLD", "ticker": null, "quantity": null, "order_type": "MARKET", "limit_price": null, "reasoning": "Waiting for dip"}"#;
            let decision = ResponseParser::parse(input).unwrap();
            assert_eq!(decision.action, Action::Hold);
        }
    }

    mod plan_parser {
        use super::*;

        #[test]
        fn parse_plan_single_action_fallback() {
            let input = r#"```json
{
  "action": "HOLD",
  "ticker": null,
  "quantity": null,
  "order_type": "MARKET",
  "limit_price": null,
  "reasoning": "No opportunities"
}
```"#;
            let plan = ResponseParser::parse_plan(input).unwrap();
            assert_eq!(plan.actions.len(), 1);
            assert_eq!(plan.actions[0].action, Action::Hold);
        }

        #[test]
        fn parse_plan_single_buy() {
            let input = r#"```json
{
  "action": "BUY",
  "ticker": "AAPL",
  "quantity": 10,
  "order_type": "MARKET",
  "limit_price": null,
  "reasoning": "Strong momentum"
}
```"#;
            let plan = ResponseParser::parse_plan(input).unwrap();
            assert_eq!(plan.actions.len(), 1);
            assert_eq!(plan.actions[0].action, Action::Buy);
            assert_eq!(plan.actions[0].ticker.as_deref(), Some("AAPL"));
        }

        #[test]
        fn parse_plan_sell_then_buy() {
            let input = r#"```json
{
  "actions": [
    {
      "action": "SELL",
      "ticker": "MSFT",
      "quantity": 5,
      "order_type": "MARKET",
      "limit_price": null,
      "reasoning": "Free up cash for better opportunity"
    },
    {
      "action": "BUY",
      "ticker": "NVDA",
      "quantity": 3,
      "order_type": "MARKET",
      "limit_price": null,
      "reasoning": "Strong AI momentum"
    }
  ]
}
```"#;
            let plan = ResponseParser::parse_plan(input).unwrap();
            assert_eq!(plan.actions.len(), 2);
            assert_eq!(plan.actions[0].action, Action::Sell);
            assert_eq!(plan.actions[0].ticker.as_deref(), Some("MSFT"));
            assert_eq!(plan.actions[1].action, Action::Buy);
            assert_eq!(plan.actions[1].ticker.as_deref(), Some("NVDA"));
        }

        #[test]
        fn parse_plan_rejects_buy_then_sell_order() {
            let input = r#"{
  "actions": [
    {
      "action": "BUY",
      "ticker": "AAPL",
      "quantity": 10,
      "order_type": "MARKET",
      "limit_price": null,
      "reasoning": "Buy first"
    },
    {
      "action": "SELL",
      "ticker": "MSFT",
      "quantity": 5,
      "order_type": "MARKET",
      "limit_price": null,
      "reasoning": "Sell second"
    }
  ]
}"#;
            let err = ResponseParser::parse_plan(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn parse_plan_rejects_three_actions() {
            let input = r#"{
  "actions": [
    {"action": "SELL", "ticker": "A", "quantity": 1, "order_type": "MARKET", "limit_price": null, "reasoning": "sell a"},
    {"action": "SELL", "ticker": "B", "quantity": 1, "order_type": "MARKET", "limit_price": null, "reasoning": "sell b"},
    {"action": "BUY", "ticker": "C", "quantity": 1, "order_type": "MARKET", "limit_price": null, "reasoning": "buy c"}
  ]
}"#;
            let err = ResponseParser::parse_plan(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn parse_plan_rejects_empty_actions() {
            let input = r#"{"actions": []}"#;
            let err = ResponseParser::parse_plan(input).unwrap_err();
            assert!(matches!(err, ParseError::ValidationFailed(_)));
        }

        #[test]
        fn parse_plan_single_action_in_array() {
            let input = r#"{
  "actions": [
    {
      "action": "SELL",
      "ticker": "AAPL",
      "quantity": 10,
      "order_type": "MARKET",
      "limit_price": null,
      "reasoning": "Take profits"
    }
  ]
}"#;
            let plan = ResponseParser::parse_plan(input).unwrap();
            assert_eq!(plan.actions.len(), 1);
            assert_eq!(plan.actions[0].action, Action::Sell);
        }

        #[test]
        fn parse_plan_with_limit_orders() {
            let input = r#"{
  "actions": [
    {
      "action": "SELL",
      "ticker": "MSFT",
      "quantity": 5,
      "order_type": "LIMIT",
      "limit_price": "410.00",
      "reasoning": "Sell at resistance"
    },
    {
      "action": "BUY",
      "ticker": "AAPL",
      "quantity": 10,
      "order_type": "LIMIT",
      "limit_price": "175.50",
      "reasoning": "Buy at support"
    }
  ]
}"#;
            let plan = ResponseParser::parse_plan(input).unwrap();
            assert_eq!(plan.actions.len(), 2);
            assert_eq!(plan.actions[0].order_type, LlmOrderType::Limit);
            assert_eq!(
                plan.actions[0].limit_price,
                Some(Decimal::from_str("410.00").unwrap())
            );
            assert_eq!(plan.actions[1].order_type, LlmOrderType::Limit);
            assert_eq!(
                plan.actions[1].limit_price,
                Some(Decimal::from_str("175.50").unwrap())
            );
        }
    }
}
