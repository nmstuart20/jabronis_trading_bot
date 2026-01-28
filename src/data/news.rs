use crate::config::DataSourcesConfig;
use crate::error::{BotError, Result};
use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;

#[derive(Debug, Clone)]
pub struct NewsItem {
    pub headline: String,
    pub summary: String,
    pub source: String,
    pub published_at: DateTime<Utc>,
    pub symbols: Vec<String>,
}

pub struct NewsService {
    http: reqwest::Client,
    api_key: Option<String>,
}

impl NewsService {
    pub fn new(config: &DataSourcesConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: config
                .news_api_key
                .as_ref()
                .map(|k| k.expose_secret().to_string()),
        }
    }

    pub async fn get_news(&self, symbols: &[&str], limit: usize) -> Result<Vec<NewsItem>> {
        let api_key = match &self.api_key {
            Some(k) => k,
            None => {
                tracing::debug!("No news API key configured, skipping news fetch");
                return Ok(vec![]);
            }
        };

        let query = symbols.join(" OR ");
        let resp = self
            .http
            .get("https://newsapi.org/v2/everything")
            .query(&[
                ("q", &query),
                ("apiKey", api_key),
                ("pageSize", &limit.to_string()),
                ("sortBy", &"publishedAt".to_string()),
                ("language", &"en".to_string()),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(BotError::Other(format!(
                "News API error {status}: {body}"
            )));
        }

        let data: serde_json::Value = resp.json().await?;
        let articles = data["articles"].as_array().cloned().unwrap_or_default();

        let items = articles
            .into_iter()
            .filter_map(|a| {
                Some(NewsItem {
                    headline: a["title"].as_str()?.to_string(),
                    summary: a["description"].as_str().unwrap_or("").to_string(),
                    source: a["source"]["name"].as_str().unwrap_or("unknown").to_string(),
                    published_at: a["publishedAt"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(Utc::now),
                    symbols: symbols.iter().map(|s| s.to_string()).collect(),
                })
            })
            .collect();

        Ok(items)
    }
}
