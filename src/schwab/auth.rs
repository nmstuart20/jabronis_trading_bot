use crate::config::SchwabConfig;
use crate::error::{BotError, Result};
use secrecy::{ExposeSecret, SecretString};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const AUTH_URL: &str = "https://api.schwabapi.com/v1/oauth/authorize";
const TOKEN_URL: &str = "https://api.schwabapi.com/v1/oauth/token";

/// Access tokens last 30 minutes.
const ACCESS_TOKEN_LIFETIME_SECS: i64 = 1800;
/// Refresh tokens last 7 days.
const REFRESH_TOKEN_LIFETIME_SECS: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone)]
struct TokenData {
    access_token: SecretString,
    refresh_token: SecretString,
    access_token_issued: chrono::DateTime<chrono::Utc>,
    refresh_token_issued: chrono::DateTime<chrono::Utc>,
}

/// On-disk representation of stored tokens.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredTokens {
    access_token: String,
    refresh_token: String,
    access_token_issued: chrono::DateTime<chrono::Utc>,
    refresh_token_issued: chrono::DateTime<chrono::Utc>,
}

fn default_token_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".schwab_bot")
        .join("tokens.json")
}

pub struct TokenManager {
    client_id: SecretString,
    client_secret: SecretString,
    redirect_uri: String,
    http: reqwest::Client,
    token_data: Arc<RwLock<Option<TokenData>>>,
    token_path: PathBuf,
}

impl TokenManager {
    pub async fn new(config: &SchwabConfig) -> Result<Self> {
        let manager = Self {
            client_id: config.app_key.clone(),
            client_secret: config.app_secret.clone(),
            redirect_uri: config.redirect_uri.clone(),
            http: reqwest::Client::new(),
            token_data: Arc::new(RwLock::new(None)),
            token_path: default_token_path(),
        };

        // Try to load persisted tokens
        if let Err(e) = manager.load_tokens().await {
            tracing::debug!("No stored tokens found: {e}");
        }

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

    /// Check if tokens need refreshing and handle it.
    /// - If no tokens exist, return TokenExpired (caller should initiate auth flow).
    /// - If access token is expiring soon, refresh it.
    /// - If refresh token is expiring soon, require full re-auth.
    pub async fn refresh_if_needed(&self) -> Result<()> {
        let state = {
            let data = self.token_data.read().await;
            match &*data {
                None => return Err(BotError::TokenExpired),
                Some(td) => {
                    let now = chrono::Utc::now();
                    let access_age = (now - td.access_token_issued).num_seconds();
                    let refresh_age = (now - td.refresh_token_issued).num_seconds();

                    if refresh_age >= REFRESH_TOKEN_LIFETIME_SECS - 60 {
                        TokenState::RefreshExpired
                    } else if access_age >= ACCESS_TOKEN_LIFETIME_SECS - 60 {
                        TokenState::AccessExpired
                    } else {
                        TokenState::Valid
                    }
                }
            }
        };

        match state {
            TokenState::Valid => Ok(()),
            TokenState::AccessExpired => {
                tracing::info!("Access token expiring, refreshing...");
                self.do_refresh_token().await
            }
            TokenState::RefreshExpired => {
                tracing::warn!("Refresh token expired, re-authentication required");
                // Clear stale tokens
                *self.token_data.write().await = None;
                Err(BotError::TokenExpired)
            }
        }
    }

    async fn do_refresh_token(&self) -> Result<()> {
        let refresh_tok = {
            let data = self.token_data.read().await;
            match &*data {
                Some(td) => td.refresh_token.clone(),
                None => return Err(BotError::TokenExpired),
            }
        };

        let credentials = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!(
                "{}:{}",
                self.client_id.expose_secret(),
                self.client_secret.expose_secret()
            ),
        );

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
        // If we already have valid tokens, just refresh if needed
        {
            let data = self.token_data.read().await;
            if data.is_some() {
                drop(data);
                match self.refresh_if_needed().await {
                    Ok(()) => {
                        tracing::info!("Using stored tokens (refreshed if needed)");
                        return Ok(());
                    }
                    Err(BotError::TokenExpired) => {
                        tracing::info!("Stored tokens expired, starting fresh auth flow");
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        let auth_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}",
            AUTH_URL,
            self.client_id.expose_secret(),
            url::form_urlencoded::byte_serialize(self.redirect_uri.as_bytes()).collect::<String>(),
        );

        // Parse host and port from redirect_uri
        let parsed_uri = url::Url::parse(&self.redirect_uri)
            .map_err(|e| BotError::OAuth(format!("Invalid redirect_uri: {e}")))?;
        let host = parsed_uri.host_str().expect("redirect_uri not set");
        let port = parsed_uri.port().unwrap_or(443);
        let bind_addr: std::net::SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| BotError::Other(format!("Invalid bind address: {e}")))?;

        // Generate a self-signed TLS certificate for the callback server
        let cert = rcgen::generate_simple_self_signed(vec![host.to_string()])
            .map_err(|e| BotError::Other(format!("Failed to generate TLS cert: {e}")))?;
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem(
            cert.cert.pem().into_bytes(),
            cert.signing_key.serialize_pem().into_bytes(),
        )
        .await
        .map_err(|e| BotError::Other(format!("Failed to configure TLS: {e}")))?;

        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let callback_path = parsed_uri.path().to_string();
        let app = axum::Router::new().route(
            &callback_path,
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

        tracing::info!("Starting HTTPS callback server on {bind_addr}");
        println!("Starting HTTPS callback server on {bind_addr}...");

        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();

        tokio::spawn(async move {
            axum_server::bind_rustls(bind_addr, tls_config)
                .handle(handle)
                .serve(app.into_make_service())
                .await
                .ok();
        });

        println!("Opening browser for Schwab login...");
        println!("If it doesn't open, visit this URL:\n{auth_url}\n");
        let _ = open::that(&auth_url);

        let code = rx
            .await
            .map_err(|_| BotError::OAuth("Failed to receive auth code".into()))?;

        // Shut down the callback server
        shutdown_handle.shutdown();

        self.exchange_code(&code).await
    }

    async fn exchange_code(&self, code: &str) -> Result<()> {
        let credentials = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!(
                "{}:{}",
                self.client_id.expose_secret(),
                self.client_secret.expose_secret()
            ),
        );

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

        let now = chrono::Utc::now();

        // Preserve the original refresh_token_issued if we're just refreshing the access token
        // (the refresh token doesn't change on access token refresh)
        let refresh_token_issued = {
            let data = self.token_data.read().await;
            if let Some(td) = &*data {
                // If the refresh token is the same, keep the original issue time
                if td.refresh_token.expose_secret() == refresh_token {
                    td.refresh_token_issued
                } else {
                    now
                }
            } else {
                now
            }
        };

        let token_data = TokenData {
            access_token: SecretString::from(access_token.to_string()),
            refresh_token: SecretString::from(refresh_token.to_string()),
            access_token_issued: now,
            refresh_token_issued,
        };

        // Persist to disk
        self.save_tokens(&token_data)?;

        let mut data = self.token_data.write().await;
        *data = Some(token_data);
        Ok(())
    }

    fn save_tokens(&self, td: &TokenData) -> Result<()> {
        let stored = StoredTokens {
            access_token: td.access_token.expose_secret().to_string(),
            refresh_token: td.refresh_token.expose_secret().to_string(),
            access_token_issued: td.access_token_issued,
            refresh_token_issued: td.refresh_token_issued,
        };

        if let Some(parent) = self.token_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| BotError::Other(format!("Failed to create token dir: {e}")))?;
        }

        let json = serde_json::to_string_pretty(&stored)?;
        std::fs::write(&self.token_path, json)
            .map_err(|e| BotError::Other(format!("Failed to write tokens: {e}")))?;

        // Restrict file permissions to owner-only (unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&self.token_path, perms);
        }

        tracing::info!("Tokens saved to {}", self.token_path.display());
        Ok(())
    }

    async fn load_tokens(&self) -> Result<()> {
        let json = std::fs::read_to_string(&self.token_path)
            .map_err(|e| BotError::Other(format!("Failed to read tokens: {e}")))?;
        let stored: StoredTokens = serde_json::from_str(&json)?;

        // Check if refresh token is still valid
        let refresh_age =
            (chrono::Utc::now() - stored.refresh_token_issued).num_seconds();
        if refresh_age >= REFRESH_TOKEN_LIFETIME_SECS {
            tracing::info!("Stored refresh token has expired, need fresh auth");
            return Err(BotError::TokenExpired);
        }

        let token_data = TokenData {
            access_token: SecretString::from(stored.access_token),
            refresh_token: SecretString::from(stored.refresh_token),
            access_token_issued: stored.access_token_issued,
            refresh_token_issued: stored.refresh_token_issued,
        };

        let access_remaining =
            ACCESS_TOKEN_LIFETIME_SECS - (chrono::Utc::now() - token_data.access_token_issued).num_seconds();
        let refresh_remaining =
            REFRESH_TOKEN_LIFETIME_SECS - (chrono::Utc::now() - token_data.refresh_token_issued).num_seconds();
        tracing::info!(
            "Loaded stored tokens (access expires in {}s, refresh expires in {}s)",
            access_remaining,
            refresh_remaining
        );

        let mut data = self.token_data.write().await;
        *data = Some(token_data);
        Ok(())
    }
}

enum TokenState {
    Valid,
    AccessExpired,
    RefreshExpired,
}
