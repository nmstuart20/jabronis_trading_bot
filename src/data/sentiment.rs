use crate::config::DataSourcesConfig;
use crate::error::Result;
use secrecy::ExposeSecret;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SentimentScore {
    pub symbol: String,
    pub score: f64,
    pub volume: u32,
    pub source: String,
}

pub struct SentimentService {
    http: reqwest::Client,
    api_key: Option<String>,
}

impl SentimentService {
    pub fn new(config: &DataSourcesConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: config
                .sentiment_api_key
                .as_ref()
                .map(|k| k.expose_secret().to_string()),
        }
    }

    pub async fn get_sentiment_scores(
        &self,
        symbols: &[&str],
    ) -> Result<HashMap<String, SentimentScore>> {
        let _api_key = match &self.api_key {
            Some(k) => k,
            None => {
                tracing::debug!("No sentiment API key configured, returning neutral scores");
                let mut scores = HashMap::new();
                for &sym in symbols {
                    scores.insert(
                        sym.to_string(),
                        SentimentScore {
                            symbol: sym.to_string(),
                            score: 0.0,
                            volume: 0,
                            source: "none".to_string(),
                        },
                    );
                }
                return Ok(scores);
            }
        };

        // Placeholder for actual sentiment API integration
        // Would call an API like Finnhub, Alpha Vantage, etc.
        let _ = &self.http;
        let mut scores = HashMap::new();
        for &sym in symbols {
            scores.insert(
                sym.to_string(),
                SentimentScore {
                    symbol: sym.to_string(),
                    score: 0.0,
                    volume: 0,
                    source: "placeholder".to_string(),
                },
            );
        }
        Ok(scores)
    }
}
