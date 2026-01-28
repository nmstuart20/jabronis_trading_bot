use crate::data::news::NewsItem;
use crate::data::sentiment::SentimentScore;
use crate::schwab::models::{Candle, Quote};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct SanitizedNews {
    pub headline: String,
    pub summary: String,
    pub source: String,
    pub published_at: DateTime<Utc>,
    pub symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MarketContext {
    pub timestamp: DateTime<Utc>,
    pub quotes: Vec<QuoteSummary>,
    pub sentiment_scores: Vec<SentimentSummary>,
    pub news_summaries: Vec<NewsSummary>,
    pub technical_indicators: Vec<TechnicalSummary>,
}

#[derive(Debug, Serialize)]
pub struct QuoteSummary {
    pub symbol: String,
    pub last_price: Decimal,
    pub bid: Decimal,
    pub ask: Decimal,
    pub volume: u64,
    pub day_high: Decimal,
    pub day_low: Decimal,
    pub change_from_open: Decimal,
}

#[derive(Debug, Serialize)]
pub struct SentimentSummary {
    pub symbol: String,
    pub score: f64,
    pub volume: u32,
}

#[derive(Debug, Serialize)]
pub struct NewsSummary {
    pub headline: String,
    pub source: String,
    pub age_minutes: i64,
}

#[derive(Debug, Serialize)]
pub struct TechnicalSummary {
    pub symbol: String,
    pub sma_5: f64,
    pub sma_10: f64,
    pub recent_trend: String,
}

pub struct InputSanitizer;

impl InputSanitizer {
    pub fn sanitize_news(news: &[NewsItem]) -> Vec<SanitizedNews> {
        news.iter()
            .map(|item| SanitizedNews {
                headline: Self::sanitize_text(&item.headline, 200),
                summary: Self::sanitize_text(&item.summary, 500),
                source: Self::sanitize_text(&item.source, 50),
                published_at: item.published_at,
                symbols: item.symbols.clone(),
            })
            .collect()
    }

    fn sanitize_text(text: &str, max_len: usize) -> String {
        text.replace("ignore", "")
            .replace("IGNORE", "")
            .replace("disregard", "")
            .replace("DISREGARD", "")
            .replace("instruction", "")
            .replace("INSTRUCTION", "")
            .replace("system", "")
            .replace("SYSTEM", "")
            .replace(['<', '>', '{', '}'], "")
            .chars()
            .take(max_len)
            .collect()
    }

    pub fn structure_market_context(
        quotes: &HashMap<String, Quote>,
        historical: &HashMap<String, Vec<Candle>>,
        sentiment: &HashMap<String, SentimentScore>,
        news: &[SanitizedNews],
    ) -> MarketContext {
        let quote_summaries: Vec<QuoteSummary> = quotes
            .values()
            .map(|q| QuoteSummary {
                symbol: q.symbol.clone(),
                last_price: q.last_price,
                bid: q.bid_price,
                ask: q.ask_price,
                volume: q.total_volume,
                day_high: q.high_price,
                day_low: q.low_price,
                change_from_open: q.last_price - q.open_price,
            })
            .collect();

        let sentiment_summaries: Vec<SentimentSummary> = sentiment
            .values()
            .map(|s| SentimentSummary {
                symbol: s.symbol.clone(),
                score: s.score,
                volume: s.volume,
            })
            .collect();

        let now = Utc::now();
        let news_summaries: Vec<NewsSummary> = news
            .iter()
            .map(|n| NewsSummary {
                headline: n.headline.clone(),
                source: n.source.clone(),
                age_minutes: (now - n.published_at).num_minutes(),
            })
            .collect();

        let technical_indicators: Vec<TechnicalSummary> = historical
            .iter()
            .map(|(sym, candles)| {
                let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
                let sma_5 = if closes.len() >= 5 {
                    closes[closes.len() - 5..].iter().sum::<f64>() / 5.0
                } else {
                    0.0
                };
                let sma_10 = if closes.len() >= 10 {
                    closes[closes.len() - 10..].iter().sum::<f64>() / 10.0
                } else {
                    0.0
                };
                let trend = if sma_5 > sma_10 {
                    "bullish"
                } else if sma_5 < sma_10 {
                    "bearish"
                } else {
                    "neutral"
                };
                TechnicalSummary {
                    symbol: sym.clone(),
                    sma_5,
                    sma_10,
                    recent_trend: trend.to_string(),
                }
            })
            .collect();

        MarketContext {
            timestamp: now,
            quotes: quote_summaries,
            sentiment_scores: sentiment_summaries,
            news_summaries,
            technical_indicators,
        }
    }
}
