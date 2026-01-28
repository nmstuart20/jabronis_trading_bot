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
    pub fn parse(response: &str) -> Result<LlmDecision, ParseError> {
        let json_str = Self::extract_json(response)?;
        let decision: LlmDecision =
            serde_json::from_str(json_str).map_err(|e| ParseError::InvalidJson(e.to_string()))?;
        Self::validate(&decision)?;
        Ok(decision)
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
