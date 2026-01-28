use crate::error::Result;
use crate::schwab::client::SchwabClient;
use crate::schwab::models::Quote;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;

struct CachedQuote {
    quote: Quote,
    fetched_at: chrono::DateTime<chrono::Utc>,
}

pub struct QuoteService {
    schwab: Arc<SchwabClient>,
    cache: DashMap<String, CachedQuote>,
    cache_ttl_secs: i64,
}

impl QuoteService {
    pub fn new(schwab: Arc<SchwabClient>) -> Self {
        Self {
            schwab,
            cache: DashMap::new(),
            cache_ttl_secs: 10,
        }
    }

    pub async fn get_current_quotes(&self, symbols: &[&str]) -> Result<HashMap<String, Quote>> {
        let now = chrono::Utc::now();

        // Find which symbols need fetching
        let mut need_fetch = Vec::new();
        let mut result = HashMap::new();

        for &sym in symbols {
            if let Some(cached) = self.cache.get(sym) {
                let age = (now - cached.fetched_at).num_seconds();
                if age < self.cache_ttl_secs {
                    result.insert(sym.to_string(), cached.quote.clone());
                    continue;
                }
            }
            need_fetch.push(sym);
        }

        if !need_fetch.is_empty() {
            let fresh = self.schwab.get_quotes(&need_fetch).await?;
            for (sym, quote) in fresh {
                self.cache.insert(
                    sym.clone(),
                    CachedQuote {
                        quote: quote.clone(),
                        fetched_at: now,
                    },
                );
                result.insert(sym, quote);
            }
        }

        Ok(result)
    }
}
