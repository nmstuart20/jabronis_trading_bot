use crate::config::SchwabConfig;
use crate::error::{BotError, Result};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tokio::sync::RwLock;

const AUTH_URL: &str = "https://api.schwabapi.com/v1/oauth/authorize";
const TOKEN_URL: &str = "https://api.schwabapi.com/v1/oauth/token";

#[derive(Debug, Clone)]
struct TokenData {
    access_token: SecretString,
    refresh_token: SecretString,
    expires_at: chrono::DateTime<chrono::Utc>,
}

pub struct TokenManager {
    client_id: SecretString,
    client_secret: SecretString,
    redirect_uri: String,
    http: reqwest::Client,
    token_data: Arc<RwLock<Option<TokenData>>>,
}

impl TokenManager {
    pub async fn new(config: &SchwabConfig) -> Result<Self> {
        let manager = Self {
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            redirect_uri: config.redirect_uri.clone(),
            http: reqwest::Client::new(),
            token_data: Arc::new(RwLock::new(None)),
        };
        Ok(manager)
    }

    pub async fn get_access_token(&self) -> Result<SecretString> {
        self.refresh_if_needed().await?;
        let data = self.token_data.read().await;
        match &*data {
            Some(td) => Ok(td.access_token.clone()),
            None => Err(BotError::TokenExpired),
        }
    }

    pub async fn refresh_if_needed(&self) -> Result<()> {
        let needs_refresh = {
            let data = self.token_data.read().await;
            match &*data {
                None => return Err(BotError::TokenExpired),
                Some(td) => chrono::Utc::now() >= td.expires_at - chrono::Duration::minutes(5),
            }
        };

        if needs_refresh {
            self.refresh_token().await?;
        }
        Ok(())
    }

    async fn refresh_token(&self) -> Result<()> {
        let refresh_tok = {
            let data = self.token_data.read().await;
            match &*data {
                Some(td) => td.refresh_token.clone(),
                None => return Err(BotError::TokenExpired),
            }
        };

        let credentials =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD,
                format!("{}:{}", self.client_id.expose_secret(), self.client_secret.expose_secret()));

        let resp = self
            .http
            .post(TOKEN_URL)
            .header("Authorization", format!("Basic {credentials}"))
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_tok.expose_secret()),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(BotError::SchwabApi {
                status,
                message: body,
            });
        }

        let token_resp: serde_json::Value = resp.json().await?;
        self.store_token_response(&token_resp).await
    }

    pub async fn initiate_auth_flow(&self) -> Result<()> {
        let auth_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}",
            AUTH_URL,
            self.client_id.expose_secret(),
            url::form_urlencoded::byte_serialize(self.redirect_uri.as_bytes()).collect::<String>(),
        );

        tracing::info!("Opening browser for authentication...");
        let _ = open::that(&auth_url);

        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let app = axum::Router::new().route(
            "/callback",
            axum::routing::get({
                let tx = tx.clone();
                move |params: axum::extract::Query<std::collections::HashMap<String, String>>| {
                    let tx = tx.clone();
                    async move {
                        if let Some(code) = params.get("code") {
                            if let Some(tx) = tx.lock().await.take() {
                                let _ = tx.send(code.clone());
                            }
                        }
                        "Authentication successful! You can close this window."
                    }
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
            .await
            .map_err(|e| BotError::Other(format!("Failed to bind callback server: {e}")))?;

        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let code = rx
            .await
            .map_err(|_| BotError::OAuth("Failed to receive auth code".into()))?;

        self.exchange_code(&code).await
    }

    async fn exchange_code(&self, code: &str) -> Result<()> {
        let credentials =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD,
                format!("{}:{}", self.client_id.expose_secret(), self.client_secret.expose_secret()));

        let resp = self
            .http
            .post(TOKEN_URL)
            .header("Authorization", format!("Basic {credentials}"))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.redirect_uri),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(BotError::SchwabApi {
                status,
                message: body,
            });
        }

        let token_resp: serde_json::Value = resp.json().await?;
        self.store_token_response(&token_resp).await
    }

    async fn store_token_response(&self, resp: &serde_json::Value) -> Result<()> {
        let access_token = resp["access_token"]
            .as_str()
            .ok_or_else(|| BotError::OAuth("Missing access_token".into()))?;
        let refresh_token = resp["refresh_token"]
            .as_str()
            .ok_or_else(|| BotError::OAuth("Missing refresh_token".into()))?;
        let expires_in = resp["expires_in"]
            .as_i64()
            .ok_or_else(|| BotError::OAuth("Missing expires_in".into()))?;

        let token_data = TokenData {
            access_token: SecretString::from(access_token.to_string()),
            refresh_token: SecretString::from(refresh_token.to_string()),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(expires_in),
        };

        let mut data = self.token_data.write().await;
        *data = Some(token_data);
        Ok(())
    }
}
