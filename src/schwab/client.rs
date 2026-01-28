use crate::config::SchwabConfig;
use crate::error::{BotError, Result};
use crate::schwab::auth::TokenManager;
use crate::schwab::models::*;
use secrecy::ExposeSecret;
use std::collections::HashMap;

const BASE_URL: &str = "https://api.schwabapi.com/trader/v1";

pub struct SchwabClient {
    http: reqwest::Client,
    token_manager: TokenManager,
    base_url: String,
}

impl SchwabClient {
    pub async fn new(config: &SchwabConfig) -> Result<Self> {
        let token_manager = TokenManager::new(config).await?;
        Ok(Self {
            http: reqwest::Client::new(),
            token_manager,
            base_url: BASE_URL.to_string(),
        })
    }

    pub async fn ensure_authenticated(&self) -> Result<()> {
        self.token_manager.initiate_auth_flow().await
    }

    async fn auth_header(&self) -> Result<String> {
        let token = self.token_manager.get_access_token().await?;
        Ok(format!("Bearer {}", token.expose_secret()))
    }

    async fn check_response(&self, resp: reqwest::Response) -> Result<reqwest::Response> {
        if resp.status().is_success() {
            Ok(resp)
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(BotError::SchwabApi {
                status,
                message: body,
            })
        }
    }

    pub async fn get_accounts(&self) -> Result<Vec<Account>> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .get(format!("{}/accounts", self.base_url))
            .header("Authorization", &auth)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn get_account(&self, account_id: &str) -> Result<Account> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .get(format!("{}/accounts/{}", self.base_url, account_id))
            .header("Authorization", &auth)
            .query(&[("fields", "positions")])
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn get_quote(&self, symbol: &str) -> Result<Quote> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .get(format!(
                "https://api.schwabapi.com/marketdata/v1/{}/quotes",
                symbol
            ))
            .header("Authorization", &auth)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        let data: HashMap<String, serde_json::Value> = resp.json().await?;
        let quote_val = data
            .get(symbol)
            .ok_or_else(|| BotError::Other(format!("No quote for {symbol}")))?;
        let quote: Quote = serde_json::from_value(quote_val["quote"].clone())?;
        Ok(quote)
    }

    pub async fn get_quotes(&self, symbols: &[&str]) -> Result<HashMap<String, Quote>> {
        let auth = self.auth_header().await?;
        let symbols_str = symbols.join(",");
        let resp = self
            .http
            .get("https://api.schwabapi.com/marketdata/v1/quotes")
            .header("Authorization", &auth)
            .query(&[("symbols", &symbols_str)])
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        let data: HashMap<String, serde_json::Value> = resp.json().await?;
        let mut quotes = HashMap::new();
        for (sym, val) in &data {
            if let Ok(q) = serde_json::from_value::<Quote>(val["quote"].clone()) {
                quotes.insert(sym.clone(), q);
            }
        }
        Ok(quotes)
    }

    pub async fn get_price_history(
        &self,
        symbol: &str,
        period_type: PeriodType,
        period: u32,
        frequency_type: FrequencyType,
        frequency: u32,
    ) -> Result<PriceHistory> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .get("https://api.schwabapi.com/marketdata/v1/pricehistory".to_string())
            .header("Authorization", &auth)
            .query(&[
                ("symbol", symbol),
                (
                    "periodType",
                    serde_json::to_value(period_type)
                        .unwrap()
                        .as_str()
                        .unwrap_or("day"),
                ),
                ("period", &period.to_string()),
                (
                    "frequencyType",
                    serde_json::to_value(frequency_type)
                        .unwrap()
                        .as_str()
                        .unwrap_or("daily"),
                ),
                ("frequency", &frequency.to_string()),
            ])
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn place_order(&self, account_id: &str, order: &Order) -> Result<OrderResponse> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .post(format!("{}/accounts/{}/orders", self.base_url, account_id))
            .header("Authorization", &auth)
            .json(order)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        // Schwab returns order ID in Location header
        let order_id = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .and_then(|loc| loc.rsplit('/').next())
            .unwrap_or("unknown")
            .to_string();
        Ok(OrderResponse { order_id })
    }

    pub async fn get_orders(&self, account_id: &str) -> Result<Vec<Order>> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .get(format!("{}/accounts/{}/orders", self.base_url, account_id))
            .header("Authorization", &auth)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn cancel_order(&self, account_id: &str, order_id: &str) -> Result<()> {
        let auth = self.auth_header().await?;
        let resp = self
            .http
            .delete(format!(
                "{}/accounts/{}/orders/{}",
                self.base_url, account_id, order_id
            ))
            .header("Authorization", &auth)
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    pub async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>> {
        let account = self.get_account(account_id).await?;
        // Positions come from account data - for now return empty
        // The actual Schwab API returns positions nested in the account response
        let _ = account;
        Ok(vec![])
    }
}
