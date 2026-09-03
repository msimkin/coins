//! CoinGecko client. Works with no API key at all (5-15 req/min); an optional
//! free demo key in the config raises that to 100 req/min.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const BASE: &str = "https://api.coingecko.com/api/v3";

/// $COINS_API_BASE points at a mirror or proxy instead of CoinGecko itself.
fn base() -> String {
    std::env::var("COINS_API_BASE")
        .ok()
        .map(|b| b.trim_end_matches('/').to_string())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| BASE.to_string())
}

/// One row of `/coins/markets`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Market {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub current_price: Option<f64>,
    pub market_cap: Option<f64>,
    /// CoinGecko's own place in the whole market, which is what a rank column
    /// should say: a tracked coin outside the fifty shown is `174`, not `51`.
    pub market_cap_rank: Option<u32>,
    pub total_volume: Option<f64>,
    pub ath: Option<f64>,
    pub ath_change_percentage: Option<f64>,
    pub last_updated: Option<String>,
    pub sparkline_in_7d: Option<Sparkline>,
    /// `price_change_percentage_*_in_currency` and everything else we don't
    /// name explicitly. Kept as raw JSON so the shape can't break us.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Sparkline {
    pub price: Vec<f64>,
}

impl Market {
    /// Percentage change for a column key like "24h" or "7d".
    pub fn change(&self, column: &str) -> Option<f64> {
        self.extra
            .get(&format!("price_change_percentage_{column}_in_currency"))
            .and_then(|v| v.as_f64())
    }

    pub fn ticker(&self) -> String {
        self.symbol.to_ascii_uppercase()
    }
}

/// A price history: (unix milliseconds, value).
pub type Series = Vec<(i64, f64)>;

#[derive(Debug, Deserialize)]
struct MarketChart {
    prices: Series,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchCoin {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub market_cap_rank: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    #[serde(default)]
    coins: Vec<SearchCoin>,
}

/// One entry of `/coins/list?include_platform=true`.
#[derive(Debug, Deserialize)]
struct ListCoin {
    id: String,
    #[serde(default)]
    platforms: HashMap<String, Option<String>>,
}

pub struct Api {
    agent: ureq::Agent,
    api_key: String,
}

/// Distinguishes "slow down" from "broken", because the caller renders them
/// differently when a cache is available.
#[derive(Debug)]
pub struct RateLimited;

impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "coingecko rate limit reached")
    }
}
impl std::error::Error for RateLimited {}

impl Api {
    pub fn new(api_key: &str) -> Api {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            .build();
        Api { agent: config.into(), api_key: api_key.to_string() }
    }

    fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let mut url = format!("{}{path}", base());
        if !query.is_empty() {
            url.push('?');
            for (i, (k, v)) in query.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                url.push_str(k);
                url.push('=');
                url.push_str(&encode(v));
            }
        }
        let mut req = self.agent.get(&url);
        if !self.api_key.is_empty() {
            req = req.header("x-cg-demo-api-key", &self.api_key);
        }
        let mut resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(429)) => return Err(anyhow!(RateLimited)),
            Err(ureq::Error::StatusCode(code)) => {
                bail!("coingecko returned HTTP {code} for {path}")
            }
            Err(e) => return Err(anyhow!(e).context(format!("requesting {path}"))),
        };
        resp.body_mut()
            .read_json::<T>()
            .map_err(|e| anyhow!(e).context(format!("parsing the response to {path}")))
    }

    /// Everything the table needs, for every coin, in one request.
    pub fn markets(
        &self,
        ids: &[String],
        currency: &str,
        changes: &[&str],
        sparkline: bool,
    ) -> Result<Vec<Market>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids.join(",");
        let changes = changes.join(",");
        // The 7-day series is three quarters of the response by size, so it is
        // asked for only when something on the screen draws it.
        let mut query = vec![
            ("vs_currency", currency),
            ("ids", ids.as_str()),
            ("sparkline", if sparkline { "true" } else { "false" }),
            ("per_page", "250"),
            ("precision", "full"),
        ];
        if !changes.is_empty() {
            query.push(("price_change_percentage", changes.as_str()));
        }
        self.get("/coins/markets", &query)
    }

    /// The most valuable coins, for regenerating the built-in list. No ids: the
    /// point is to find out which coins are worth carrying.
    pub fn top_markets(
        &self,
        currency: &str,
        count: usize,
        category: Option<&str>,
    ) -> Result<Vec<Market>> {
        let per_page = count.to_string();
        let mut query = vec![
            ("vs_currency", currency),
            ("order", "market_cap_desc"),
            ("per_page", per_page.as_str()),
            ("page", "1"),
        ];
        if let Some(c) = category {
            query.push(("category", c));
        }
        self.get("/coins/markets", &query)
    }

    pub fn market_chart(&self, id: &str, currency: &str, days: &str) -> Result<Series> {
        let chart: MarketChart = self.get(
            &format!("/coins/{id}/market_chart"),
            &[("vs_currency", currency), ("days", days)],
        )?;
        Ok(chart.prices)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchCoin>> {
        let r: SearchResult = self.get("/search", &[("query", query)])?;
        Ok(r.coins)
    }

    pub fn supported_currencies(&self) -> Result<Vec<String>> {
        self.get("/simple/supported_vs_currencies", &[])
    }

    /// On-chain addresses for the given coins on one chain — an ERC-20 contract
    /// on Ethereum, an SPL mint on Solana — from a single request.
    ///
    /// `/coins/list` covers every coin at once, which keeps wallet support at
    /// one API call however many tokens are tracked — well worth filtering a
    /// large response down to the handful of ids we care about.
    pub fn platform_contracts(
        &self,
        chain: &str,
        ids: &[String],
    ) -> Result<HashMap<String, String>> {
        let list: Vec<ListCoin> = self.get("/coins/list", &[("include_platform", "true")])?;
        let mut out = HashMap::new();
        for coin in list {
            if !ids.contains(&coin.id) {
                continue;
            }
            if let Some(Some(addr)) = coin.platforms.get(chain) {
                if !addr.is_empty() {
                    // Ethereum addresses are case-insensitive hex; a Solana mint
                    // is base58 and case-significant, so it must not be folded.
                    let addr = if chain == "ethereum" {
                        addr.to_ascii_lowercase()
                    } else {
                        addr.clone()
                    };
                    out.insert(coin.id, addr);
                }
            }
        }
        Ok(out)
    }
}

pub fn is_rate_limit(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.downcast_ref::<RateLimited>().is_some())
}

/// Minimal percent-encoding: our query values are ids, tickers and currencies.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `sparkline_in_7d` has no timestamps — it is hourly and ends at `last_updated`,
/// so we can reconstruct them and use it as a 7-day series for free.
/// The week of history that rides along with the prices, in the currency that was
/// asked for.
///
/// `sparkline_in_7d` comes back in dollars whatever `vs_currency` says — not
/// documented anywhere, and invisible until you draw it: in euros the curve sat
/// 16% above the quote beside it, and appending the live price put a cliff on the
/// end of every week-long chart. So the week is pinned to the quote: scaled by
/// the ratio between them, which is the exchange rate and whatever the coin did
/// since its last point, and which is 1.0 when the currency already is dollars.
/// The shape is untouched — it is one multiplier — and the curve now ends where
/// the price says it ends.
pub fn sparkline_series(m: &Market) -> Option<Series> {
    let raw = &m.sparkline_in_7d.as_ref()?.price;
    let factor = match (m.current_price, raw.last()) {
        (Some(now), Some(&last)) if now > 0.0 && last > 0.0 && now.is_finite() => now / last,
        _ => 1.0,
    };
    let prices: Vec<f64> = raw.iter().map(|p| p * factor).collect();
    if prices.len() < 2 {
        return None;
    }
    let end_ms = m
        .last_updated
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let step = 3_600_000i64;
    let n = prices.len() as i64;
    Some(
        prices
            .into_iter()
            .enumerate()
            .map(|(i, p)| (end_ms - (n - 1 - i as i64) * step, p))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_sparkline(prices: Vec<f64>, quote: f64) -> Market {
        let mut m: Market = serde_json::from_value(serde_json::json!({
            "id": "x", "symbol": "x", "name": "x",
            "current_price": quote,
            "last_updated": "2026-09-02T12:00:00.000Z",
        }))
        .unwrap();
        m.sparkline_in_7d = Some(Sparkline { price: prices });
        m
    }

    #[test]
    fn a_week_of_history_is_pinned_to_the_price_beside_it() {
        // What the API actually returns for a euro request: dollars.
        let m = with_sparkline(vec![100.0, 110.0, 105.0, 116.0], 100.0);
        let s = sparkline_series(&m).unwrap();
        // It ends where the quote says, so appending the quote cannot make a cliff.
        assert!((s.last().unwrap().1 - 100.0).abs() < 1e-9);
        // And the shape is untouched: every ratio between points is what it was.
        let v: Vec<f64> = s.iter().map(|(_, p)| *p).collect();
        assert!((v[1] / v[0] - 1.10).abs() < 1e-9, "{v:?}");
        assert!((v[2] / v[1] - 105.0 / 110.0).abs() < 1e-9, "{v:?}");
    }

    #[test]
    fn a_coin_without_a_quote_is_left_as_it_came() {
        let mut m = with_sparkline(vec![1.0, 2.0], 1.0);
        m.current_price = None;
        let s = sparkline_series(&m).unwrap();
        assert_eq!(s.iter().map(|(_, p)| *p).collect::<Vec<_>>(), vec![1.0, 2.0]);
    }
}
