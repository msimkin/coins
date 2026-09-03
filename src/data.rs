//! Assembles a renderable snapshot from the cache and, when it must, the network.
//!
//! The read path is what makes the common case feel instant: a fresh cache is
//! rendered without a request; a merely warm one is rendered immediately and
//! refreshed by a detached background process so the *next* run is fresh; only
//! a cold or long-stale cache blocks on the network. Whatever happens, the
//! header states the age of what you are looking at.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::cache::Cache;
use crate::coingecko::{Api, Market, SearchCoin, Series, is_rate_limit, sparkline_series};
use crate::coins;
use crate::config::{BalanceView, Chain, Config, Range};
use crate::portfolio::{self, HoldingSource, Portfolio};
use crate::wallet::Rpc;

/// Prices are recomputed by CoinGecko every 60s on the public API, so a
/// shorter TTL would only spend rate limit for nothing.
const MARKETS_TTL: Duration = Duration::from_secs(60);
/// Balances move far less often than prices, and the background warmer keeps
/// them current, so this can be generous.
const WALLET_TTL: Duration = Duration::from_secs(5 * 60);
const META_TTL: Duration = Duration::from_secs(30 * 86_400);
/// A cached picture younger than this is worth drawing at once and refreshing
/// behind: an hour-old price on the screen now beats a current one in fifteen
/// seconds, and the header says how old it is either way.
const STALE_LIMIT: Duration = Duration::from_secs(3600);

/// How far a snapshot may go for its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Whatever is on disk, however old, and no requests at all. The first paint
    /// of a screen is drawn this way, so that something is up immediately.
    Cache,
    /// Requests only for what is missing. Anything cached is good enough, however
    /// stale, because a warmer is bringing it up to date — this is what stops a
    /// chart past its six hours from blocking a screen for fifteen seconds.
    Gaps,
    /// Refetch anything past its age.
    Fresh,
}

/// Which of those to use, given how old the prices on disk are.
///
/// A live display refreshes itself, so it always reaches for fresh data and never
/// leaves a warmer to do it. Everything else would rather draw an hour-old
/// picture now and be current next time.
fn reach_for(age: Option<Duration>, force: bool, live: bool) -> Reach {
    match age {
        _ if force => Reach::Fresh,
        Some(age) if age < MARKETS_TTL => Reach::Gaps,
        Some(age) if age < STALE_LIMIT && !live => Reach::Gaps,
        _ => Reach::Fresh,
    }
}

/// How many of the built-in popular coins ride along with every prices request,
/// history and all, so that any of them can be shown and plotted without one.
/// Fifty is the trade: each coin's week of history is about 3 KB, so fifty cost
/// 200 KB against the 13 KB a tracked-coins-only request took. The list itself
/// holds 250, and the rest are one request away.
const POPULAR_PRICED: usize = 50;

/// Beyond this many charted coins we stop fetching history — each one is a
/// request, and the keyless allowance is 5-15 per minute.
const MAX_FACETS: usize = 8;
/// The list view wants history for every row at once. Past this, every row
/// falls back to the free 7-day series rather than spending a request each.
const MAX_INLINE_SERIES: usize = 10;

#[derive(Debug, Clone)]
pub struct Row {
    pub market: Market,
    /// Palette slot, fixed by the coin's position in the config.
    pub color: usize,
    pub series: Option<Series>,
    /// Units held, from `[holdings]` and any wallets.
    pub amount: f64,
    /// Changes worked out from a chart rather than supplied with the price —
    /// the month columns, keyed by column name.
    pub changes: BTreeMap<String, f64>,
    /// The series is the free week of history that came with the price, and the
    /// configured range is longer than that. Whatever names the period has to
    /// say `7d`, or it is describing history the row does not have.
    pub week_fallback: bool,
}

impl Row {
    /// A column's change, wherever it came from.
    pub fn change(&self, column: &str) -> Option<f64> {
        self.market
            .change(column)
            .or_else(|| self.changes.get(column).copied())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Fresh,
    /// Served from cache; a background refresh was started.
    Warming,
    Offline,
    RateLimited,
}

/// Which screen was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// `price` — the list, each row carrying its own small plot.
    List,
    /// `coins plot` — the big plots, in the configured form.
    Plot,
    /// `coins balance` — what each address holds, and what it is worth.
    Balance,
    /// `coins top` — the largest coins there are, by what they are worth.
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartPlan {
    /// No big chart: the list view's plots live in the table.
    Off,
    /// One coin, in absolute currency.
    Single,
    /// One small chart per coin, each with its own axis.
    Facets,
}

pub struct Snapshot {
    pub rows: Vec<Row>,
    pub currency: String,
    pub range: Range,
    pub plan: ChartPlan,
    /// The list view shows the table; the plot views speak for themselves.
    pub show_table: bool,
    /// Whether the addresses group appears under the table.
    pub show_addresses: bool,
    /// Which screen this is. The table needs it: `coins top` is a different
    /// shape from the others, not the same one with more rows.
    pub view: View,
    /// Where the ranked coins end and the tracked ones from further down the
    /// market begin, so the table can show the join.
    pub top_break: Option<usize>,
    /// Whether the coins group appears at all. `balance = "addresses"` turns it
    /// off for that view, so the two commands divide the screen between them.
    pub show_coins: bool,
    /// Rows to chart, as indices into `rows`.
    pub charted: Vec<usize>,
    pub age: Duration,
    pub status: Status,
    pub portfolio: Option<Portfolio>,
    pub warnings: Vec<String>,
}

impl Snapshot {
    /// True when every charted row has history for the selected range, so the
    /// trend column can be labelled with that range rather than the free
    /// 7-day sparkline it otherwise falls back to.
    pub fn trend_is_range(&self) -> bool {
        !self.rows.is_empty()
            && self.rows.iter().all(|r| r.series.is_some() && !r.week_fallback)
    }
}

pub struct Fetcher {
    pub cfg: Config,
    /// How far this snapshot may go for its data.
    reach: Reach,
    /// A display that redraws on its own schedule. It refreshes what it needs
    /// itself, so nothing is gained by spawning a warmer — and a warmer spawned
    /// once a minute by a process that never exits is a defunct child once a
    /// minute.
    live: bool,
    pub api: Api,
    pub cache: Cache,
    /// `--refresh`: ignore cached values.
    pub force: bool,
}

impl Fetcher {
    pub fn new(cfg: Config, force: bool) -> Result<Fetcher> {
        let api = Api::new(&cfg.api_key);
        let cache = Cache::new(crate::config::cache_home()?);
        Ok(Fetcher { cfg, api, cache, force, live: false, reach: Reach::Fresh })
    }

    /// A display refreshes itself, so it warms nothing and forces nothing after
    /// its first frame — `--refresh` every minute for a week is what the rate
    /// limit exists to stop.
    pub fn set_live(&mut self) {
        self.live = true;
    }

    pub fn stop_forcing(&mut self) {
        self.force = false;
    }

    /// The age of the prices on disk, which is what decides how far to reach.
    fn cached_age(&self) -> Option<Duration> {
        self.cache.get::<Vec<Market>>(&self.markets_key()).map(|h| h.age)
    }

    /// Sets the reach for what follows, and returns it. `Cache` is left alone —
    /// the first paint asks for exactly that.
    fn decide_reach(&mut self, reach: Option<Reach>) -> Reach {
        self.reach = match reach {
            Some(r) => r,
            None => reach_for(self.cached_age(), self.force, self.live),
        };
        // Stale data on the screen is only acceptable because something is on its
        // way to replace it.
        if self.reach == Reach::Gaps && !self.live {
            self.cache.spawn_warm();
        }
        self.reach
    }

    /// Checks the configured display currency against CoinGecko's list.
    pub fn validate_currency(&self) -> Result<()> {
        // The currencies people actually use are waved through, so the common
        // case never spends a request on the supported-currency list.
        if COMMON_CURRENCIES.contains(&self.cfg.currency.as_str()) {
            return Ok(());
        }
        let list: Vec<String> = match self.cached_currencies() {
            Some(l) => l,
            // Without the list we can't judge; let the request speak instead.
            None => return Ok(()),
        };
        if list.iter().any(|c| c == &self.cfg.currency) {
            return Ok(());
        }
        let near: Vec<&String> = list
            .iter()
            .filter(|c| {
                c.starts_with(self.cfg.currency.get(..1).unwrap_or("_"))
                    || c.contains(&self.cfg.currency)
            })
            .take(8)
            .collect();
        let hint = if near.is_empty() {
            String::new()
        } else {
            format!(
                " — did you mean {}?",
                near.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            )
        };
        bail!(
            "{:?} is not a currency CoinGecko can quote{hint}\nedit `currency` with `coins config --edit`",
            self.cfg.currency
        );
    }

    fn cached_currencies(&self) -> Option<Vec<String>> {
        if let Some(v) = self.cache.get_fresh::<Vec<String>>("supported-currencies", META_TTL) {
            return Some(v);
        }
        let fetched = self.api.supported_currencies().ok()?;
        self.cache.put("supported-currencies", &fetched);
        Some(fetched)
    }

    /// Turns `btc`, `bitcoin` or `Bitcoin` into a coin id, preferring something
    /// already tracked so the common case costs no request.
    pub fn resolve_coin(&self, query: &str) -> Result<String> {
        let q = query.trim().to_ascii_lowercase();
        // An address here is a category error, not a misspelling.
        if crate::config::is_wallet_address(query.trim()) {
            bail!(
                "{} is an address, and a plot is of a coin — try `coins balance` \
                 to see what your addresses hold",
                crate::wallet::short_address(query.trim())
            );
        }
        if self.cfg.coins.contains(&q) {
            return Ok(q);
        }
        // Match a tracked coin by ticker without a network round trip.
        if let Some(m) = self
            .cache
            .get::<Vec<Market>>(&self.markets_key())
            .map(|h| h.value)
            .unwrap_or_default()
            .into_iter()
            .find(|m| m.symbol.eq_ignore_ascii_case(&q) && self.cfg.coins.contains(&m.id))
        {
            return Ok(m.id);
        }
        match self.search_best(&q)? {
            Match::One(c) => Ok(c.id),
            Match::Many(cands) => bail!("{}", ambiguous_message(&q, &cands)),
            Match::Unknown(near) => bail!("{}", no_match_message(&q, &near)),
        }
    }

    /// `/search`, reduced to a decision — but the built-in list of popular
    /// coins is consulted first, so the common case needs no network at all.
    pub fn search_best(&self, query: &str) -> Result<Match> {
        let q = query.trim().to_ascii_lowercase();
        if let Some(c) = coins::resolve(&q) {
            return Ok(Match::One(SearchCoin {
                id: c.0.to_string(),
                name: c.2.to_string(),
                symbol: c.1.to_string(),
                market_cap_rank: None,
            }));
        }
        let key = format!("search-{q}");
        let coins: Vec<SearchCoin> = match self.cache.get_fresh(&key, META_TTL) {
            Some(v) => v,
            None => {
                let v = self.api.search(&q).with_context(|| {
                    format!(
                        "could not reach CoinGecko to look up {q:?}\n\
                         it is not one of the {} popular coins built in, so a lookup is needed",
                        coins::POPULAR.len()
                    )
                })?;
                self.cache.put(&key, &v);
                v
            }
        };
        if coins.is_empty() {
            return Ok(Match::Unknown(Vec::new()));
        }
        // An exact id is unambiguous by definition.
        if let Some(c) = coins.iter().find(|c| c.id.eq_ignore_ascii_case(&q)) {
            return Ok(Match::One(c.clone()));
        }
        let mut exact: Vec<SearchCoin> = coins
            .iter()
            .filter(|c| c.symbol.eq_ignore_ascii_case(&q) || c.name.eq_ignore_ascii_case(&q))
            .cloned()
            .collect();
        // Search is fuzzy: "bitcon" comes back with a list of real coins, none
        // of them what was meant. Never guess from those — suggest and stop.
        if exact.is_empty() {
            let mut near = coins.clone();
            near.sort_by_key(|c| c.market_cap_rank.unwrap_or(u32::MAX));
            return Ok(Match::Unknown(near.into_iter().take(5).collect()));
        }
        exact.sort_by_key(|c| c.market_cap_rank.unwrap_or(u32::MAX));
        let best = exact[0].clone();
        // A clear market-cap leader is the answer; a close field is a question.
        let decisive = match (best.market_cap_rank, exact.get(1).and_then(|c| c.market_cap_rank)) {
            (Some(a), Some(b)) => a * 4 < b.max(1),
            (Some(_), None) => true,
            _ => exact.len() == 1,
        };
        if decisive {
            Ok(Match::One(best))
        } else {
            Ok(Match::Many(exact.into_iter().take(5).collect()))
        }
    }

    /// The coins to ask for: the ones this screen needs, and the most valuable
    /// hundred alongside them.
    ///
    /// They cost nothing in requests — one call carries all of them — and they
    /// are why `coins btc` answers at once for a coin nobody tracks, and keeps
    /// answering while the API is rate-limiting or unreachable. The sparklines
    /// are what makes such a response big (3 KB a coin, against 1 KB for the
    /// prices), so they are asked for only when a screen can show one.
    fn fetch_ids(&self, needed: &[String]) -> Vec<String> {
        let mut ids = needed.to_vec();
        for (id, _, _) in coins::POPULAR.iter().take(POPULAR_PRICED) {
            if !ids.iter().any(|x| x == id) {
                ids.push((*id).to_string());
            }
        }
        ids
    }

    fn markets_key(&self) -> String {
        format!("markets-{}-{}", self.cfg.currency, self.cfg.market_columns().join("_"))
    }

    /// `needed` is what the screen must have; the request asks for more.
    fn markets(&self, needed: &[String]) -> (Vec<Market>, Duration, Status) {
        let key = self.markets_key();
        let changes = self.cfg.market_columns();
        let asked = self.fetch_ids(needed);
        let fetch = || self.api.markets(&asked, &self.cfg.currency, &changes, true);

        let cached = if self.force { None } else { self.cache.get::<Vec<Market>>(&key) };
        // A cached set that predates a config change is short of the coin just
        // added. It is still worth having: the coins it does carry are priced,
        // and the missing one is named in a warning. Discarding it wholesale
        // meant that adding a coin while offline, or while rate-limited, took
        // every price off the screen — including the ones already in hand.
        let complete = cached
            .as_ref()
            .is_some_and(|h| needed.iter().all(|id| h.value.iter().any(|m| &m.id == id)));

        if let Some(hit) = cached {
            if complete && hit.age < MARKETS_TTL {
                return (hit.value, hit.age, Status::Fresh);
            }
            // Cached and complete is as far as `Cache` and `Gaps` go: the first
            // is drawing what is already here, and the second has a warmer on
            // the way. `Warming` is what puts `refreshing` in the header.
            if complete && self.reach != Reach::Fresh {
                return (hit.value, hit.age, Status::Warming);
            }
            if self.reach == Reach::Cache {
                return (hit.value, hit.age, Status::Offline);
            }
            return match fetch() {
                Ok(v) => {
                    self.cache.put(&key, &v);
                    (v, Duration::ZERO, Status::Fresh)
                }
                Err(e) => {
                    let status = if is_rate_limit(&e) { Status::RateLimited } else { Status::Offline };
                    (hit.value, hit.age, status)
                }
            };
        }
        // Nothing cached at all: even the first paint has to give up here, and
        // it does so without a request, so the screen is simply not drawn yet.
        if self.reach == Reach::Cache {
            return (Vec::new(), Duration::ZERO, Status::Offline);
        }
        match fetch() {
            Ok(v) => {
                self.cache.put(&key, &v);
                (v, Duration::ZERO, Status::Fresh)
            }
            Err(e) => {
                // Which failure it was decides what the caller tells the user;
                // "check your connection" is wrong advice for a rate limit.
                let status = if is_rate_limit(&e) { Status::RateLimited } else { Status::Offline };
                (Vec::new(), Duration::ZERO, status)
            }
        }
    }

    fn series(&self, id: &str) -> Option<Series> {
        self.series_over(id, self.cfg.range)
    }

    /// The price chart for one coin over `range`. Keyed by range, so a month
    /// column and a plot of the same period share the one request.
    fn series_over(&self, id: &str, range: Range) -> Option<Series> {
        let key = format!("chart-{id}-{}-{}", self.cfg.currency, range.key());
        let ttl = range.chart_ttl();
        if !self.force {
            if let Some(v) = self.cache.get_fresh::<Series>(&key, ttl) {
                return Some(v);
            }
            // Past its age, but there: fifteen seconds of blank screen is a
            // worse answer than a chart that is a few hours old and labelled.
            if self.reach != Reach::Fresh {
                if let Some(hit) = self.cache.get::<Series>(&key) {
                    return Some(hit.value);
                }
            }
        }
        if self.reach == Reach::Cache {
            return None;
        }
        match self.api.market_chart(id, &self.cfg.currency, range.days()) {
            Ok(v) => {
                self.cache.put(&key, &v);
                Some(v)
            }
            // Fall back to any stale copy before giving up on the chart.
            Err(_) => self.cache.get::<Series>(&key).map(|h| h.value),
        }
    }

    /// The screen `view` asked for, narrowed to one coin when `focus` is set.
    /// Warnings about the prices themselves: coins the API knows nothing about,
    /// and coins too cheap to render at the configured ceiling.
    fn price_warnings(&self, needed: &[String], markets: &[Market], status: Status) -> Vec<String> {
        let mut out = Vec::new();
        for m in markets.iter().filter(|m| needed.iter().any(|id| id == &m.id)) {
            if m.current_price.is_none() {
                out.push(format!(
                    "coingecko has no price for {:?} — anything held in it cannot be valued",
                    m.id
                ));
            }
        }
        for id in needed {
            if !markets.iter().any(|m| &m.id == id) {
                // Whether the coin has no data or merely no *cached* data is the
                // difference between a name to fix and a wait.
                out.push(match status {
                    Status::Offline => {
                        format!("no price for {id:?} yet — could not reach CoinGecko")
                    }
                    Status::RateLimited => {
                        format!("no price for {id:?} yet — CoinGecko is rate-limiting this machine")
                    }
                    _ => format!("coingecko has no market data for {id:?}"),
                });
            }
        }
        // A price below the decimal ceiling prints as zero. Better to say so
        // than to show someone a coin apparently worth nothing. Only for the
        // coins this screen shows: most of the hundred fetched alongside them
        // are worth a fraction of a cent, and none of them is on the screen.
        for m in markets.iter().filter(|m| needed.iter().any(|id| id == &m.id)) {
            if let Some(p) = m.current_price {
                if crate::render::fmt::rounds_to_zero(p, self.cfg.max_decimals.max(2)) {
                    out.push(format!(
                        "{} is too small to show at max_decimals = {} — raise it to see the price",
                        m.ticker(),
                        self.cfg.max_decimals
                    ));
                }
            }
        }
        out
    }

    /// Every balance held, per wallet and merged, with the dust removed.
    ///
    /// Airdrops and contract leftovers leave most real addresses holding a few
    /// balances worth a fraction of a cent. Pruned here, once, so they reach
    /// neither the rows, nor the allocation bar, nor the total.
    fn holdings(
        &self,
        ids: &[String],
        markets: &[Market],
        warnings: &mut Vec<String>,
    ) -> (Vec<WalletBalances>, BTreeMap<String, f64>) {
        let mut per_wallet = if self.cfg.wallets.is_empty() {
            Vec::new()
        } else {
            self.wallet_amounts(ids, warnings)
        };
        for wallet in &mut per_wallet {
            wallet.amounts.retain(|id, amount| {
                let price = markets.iter().find(|m| &m.id == id).and_then(|m| m.current_price);
                match price {
                    // Dust: airdrops and contract leftovers, worth less than a
                    // hundredth of the display currency.
                    Some(price) => {
                        let value = price * *amount;
                        value > 0.0 && !crate::render::fmt::rounds_to_zero(value, 2)
                    }
                    // A holding nobody can price is not a holding worth nothing.
                    // Treating the two alike deleted it from the screen, and a
                    // config with two wallets in it was told it held nothing.
                    None => *amount > 0.0,
                }
            });
        }
        let mut merged: BTreeMap<String, f64> = BTreeMap::new();
        for wallet in &per_wallet {
            for (id, amount) in &wallet.amounts {
                *merged.entry(id.clone()).or_insert(0.0) += *amount;
            }
        }
        for (id, amount) in &self.cfg.holdings {
            *merged.entry(id.clone()).or_insert(0.0) += *amount;
        }
        (per_wallet, merged)
    }

    /// Fills in the price history each row needs, and closes the gap between
    /// that history and the live quote.
    fn attach_series(&self, rows: &mut [Row], want: &[usize]) {
        for (i, row) in rows.iter_mut().enumerate() {
            // A 1-week chart is free: it rides along with the prices request.
            if self.cfg.range == Range::W1 {
                row.series = sparkline_series(&row.market);
                if row.series.is_some() {
                    continue;
                }
            }
            if want.contains(&i) {
                row.series = self.series(&row.market.id);
            }
            // No chart for the range — a coin nobody tracks, or a request that
            // could not be made. The week that came with the price is still a
            // curve, and a curve is what was asked for; the label says which
            // week it is rather than claiming the range.
            if row.series.is_none() && want.contains(&i) {
                row.series = sparkline_series(&row.market);
                row.week_fallback = row.series.is_some() && self.cfg.range != Range::W1;
            }
        }
        // History lags the live quote by up to an hour, which would otherwise
        // put the period's low above the current price.
        for row in rows.iter_mut() {
            if let Some(series) = row.series.as_mut() {
                append_quote(series, &row.market);
            }
        }
    }

    /// The coins this screen shows, in the order it shows them — and, for the
    /// market list, where its ranked coins end and yours begin.
    ///
    /// What each view is for: plain `coins` shows exactly what you track,
    /// predictably and without hinting at holdings; the balance view adds every
    /// coin you hold, so one is never invisible merely because it is not in
    /// `coins`; and the market list is the market, whatever the config says
    /// about holdings.
    fn on_screen(
        &self,
        view: View,
        focus: Option<&String>,
        markets: &[Market],
        top_count: Option<usize>,
        show_addresses: bool,
    ) -> (Vec<String>, Option<usize>) {
        match focus {
            Some(f) => (vec![f.clone()], None),
            None if view == View::Top => {
                market_list(markets, &self.cfg.coins, top_count.unwrap_or(self.cfg.top))
            }
            None if show_addresses => (self.cfg.quoted_coins(), None),
            None => (self.cfg.coins.clone(), None),
        }
    }

    /// Everything about history in one place: which coins get a chart, which get
    /// any series at all, and the series themselves.
    fn attach_history(
        &self,
        rows: &mut [Row],
        view: View,
        focused: bool,
        inline_plots: bool,
        warnings: &mut Vec<String>,
    ) -> (ChartPlan, Vec<usize>) {
        let (plan, charted) = plan_chart(view, focused, rows.len());
        if plan == ChartPlan::Facets && charted.len() < rows.len() {
            warnings.push(format!(
                "plotting the first {} of {} coins — each one costs a request",
                charted.len(),
                rows.len()
            ));
        }
        let want: Vec<usize> = if plan == ChartPlan::Off {
            // The list view's per-row plots need history for every row, but not
            // at the price of a request each once the list gets long.
            if inline_plots && rows.len() <= MAX_INLINE_SERIES {
                (0..rows.len()).collect()
            } else {
                Vec::new()
            }
        } else {
            charted.clone()
        };
        self.attach_series(rows, &want);
        // Not for the market list: a month column is a chart per coin, and that
        // screen's whole point is that it costs nothing. It shows the columns
        // the prices request already answers.
        if view != View::Top {
            self.attach_month_changes(rows);
        }
        (plan, charted)
    }

    /// Fills the month columns, which no single request can answer: one chart
    /// per coin, the longest period asked for, sliced for the shorter ones.
    fn attach_month_changes(&self, rows: &mut [Row]) {
        let columns = self.cfg.series_columns();
        let Some(range) = self.cfg.month_chart_range() else { return };
        // Same ceiling as the inline plots: past it, a screen would cost more
        // requests than the keyless allowance gives in a minute.
        for row in rows.iter_mut().take(MAX_INLINE_SERIES) {
            let Some(series) = self.series_over(&row.market.id, range) else { continue };
            for (column, days) in &columns {
                if let Some(v) = change_over_days(&series, *days) {
                    row.changes.insert((*column).to_string(), v);
                }
            }
        }
    }

    /// The screen `view` asked for, narrowed to one coin when `focus` is set.
    /// The screen, reaching as far for its data as the state of the cache says
    /// it should — and starting a warmer when it settles for something stale.
    pub fn snapshot(
        &mut self,
        focus: Option<&str>,
        view: View,
        inline_plots: bool,
        top_count: Option<usize>,
    ) -> Result<Snapshot> {
        self.decide_reach(None);
        self.build(focus, view, inline_plots, top_count)
    }

    /// The screen from what is already on disk, and nothing else. This is the
    /// paint that goes up immediately, before a single request is made; it fails
    /// quietly when there is not enough cached to draw anything.
    pub fn cached_snapshot(
        &mut self,
        focus: Option<&str>,
        view: View,
        inline_plots: bool,
        top_count: Option<usize>,
    ) -> Result<Snapshot> {
        self.decide_reach(Some(Reach::Cache));
        self.build(focus, view, inline_plots, top_count)
    }

    fn build(
        &self,
        focus: Option<&str>,
        view: View,
        inline_plots: bool,
        top_count: Option<usize>,
    ) -> Result<Snapshot> {
        let focus_id = match focus {
            Some(q) => Some(self.resolve_coin(q)?),
            None => None,
        };
        let mut ids = self.cfg.quoted_coins();
        if let Some(f) = &focus_id {
            if !ids.contains(f) {
                ids.push(f.clone());
            }
        }
        let (markets, age, status) = self.markets(&ids);
        if markets.is_empty() {
            match status {
                Status::RateLimited => bail!(
                    "CoinGecko is rate-limiting this machine and nothing is cached yet\n{RATE_LIMIT_HINT}"
                ),
                _ => bail!(
                    "could not reach CoinGecko and no cached prices are available\n\
                     check your connection, then try again"
                ),
            }
        }
        let mut warnings = self.price_warnings(&ids, &markets, status);

        let show_addresses =
            view != View::Top && (view == View::Balance || self.cfg.show_addresses);
        let (display, top_break) =
            self.on_screen(view, focus_id.as_ref(), &markets, top_count, show_addresses);

        // Wallet warnings are kept apart, so a private screen says nothing
        // about your wallets.
        let mut wallet_warnings: Vec<String> = Vec::new();
        let (per_wallet, amounts) = self.holdings(&ids, &markets, &mut wallet_warnings);
        let wallet_trouble = !wallet_warnings.is_empty();
        if show_addresses {
            warnings.append(&mut wallet_warnings);
        }

        let mut rows = build_rows(&display, &self.cfg.coins, &markets, &amounts, focus_id.is_some());
        if rows.is_empty() {
            bail!("{}", no_rows_message(&display, status));
        }

        // `balance = "addresses"` drops the coin table from this view, but only
        // when there is something held to put in its place: an empty screen
        // would be a worse answer than a repeated one.
        let show_coins = !(view == View::Balance
            && self.cfg.balance == BalanceView::Addresses
            && rows.iter().any(|r| r.amount > 0.0));

        let (plan, charted) =
            self.attach_history(&mut rows, view, focus_id.is_some(), inline_plots, &mut warnings);

        // Each source is valued on its own, so it can have its own group of rows.
        let mut sources: Vec<HoldingSource> = per_wallet
            .iter()
            .map(|w| HoldingSource {
                label: w.label.clone(),
                address: Some(w.address.clone()),
                coins: breakdown(&rows, &w.amounts),
            })
            .collect();
        if !self.cfg.holdings.is_empty() {
            sources.push(HoldingSource {
                label: "off-chain".into(),
                address: None,
                coins: breakdown(&rows, &self.cfg.holdings.clone()),
            });
        }
        let portfolio = portfolio::build(&rows, sources);

        // Silent only when there is genuinely nothing to say: a wallet that
        // could not be read has its own warning to show, so render instead.
        if view == View::Balance && !wallet_trouble && !rows.iter().any(|r| r.amount > 0.0) {
            bail!(
                "nothing held yet — add an address with `coins add <address>`, or amounts under `[holdings]` in the config"
            );
        }

        Ok(Snapshot {
            rows,
            currency: self.cfg.currency.clone(),
            range: self.cfg.range,
            plan,
            view,
            top_break,
            show_table: plan == ChartPlan::Off,
            show_addresses,
            show_coins,
            charted,
            age,
            status,
            portfolio,
            warnings,
        })
    }

    /// Balances for every tracked coin the address can hold, kept per wallet so
    /// each address can be shown and valued on its own rows.
    fn wallet_amounts(&self, ids: &[String], warnings: &mut Vec<String>) -> Vec<WalletBalances> {
        // One token-address lookup per chain actually in use, not per wallet.
        let mut chains: Vec<Chain> = Vec::new();
        for w in &self.cfg.wallets {
            let Some(c) = w.chain() else { continue };
            if !chains.contains(&c) {
                chains.push(c);
            }
        }
        let mut tokens: Vec<(Chain, BTreeMap<String, String>)> = Vec::new();
        for chain in chains {
            // A chain with no token layer needs no lookup at all.
            if chain.platform_key().is_none() {
                continue;
            }
            let map = self.platform_contracts(chain, ids, warnings);
            tokens.push((chain, map));
        }

        let mut out: Vec<WalletBalances> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for wallet in &self.cfg.wallets {
            // The same address twice would be read twice and counted twice, and
            // a portfolio quietly worth double is the worst kind of wrong number.
            let lower = wallet.address.trim().to_ascii_lowercase();
            if seen.contains(&lower) {
                warnings.push(format!(
                    "{} is listed twice — counted once",
                    crate::wallet::short_address(&wallet.address)
                ));
                continue;
            }
            seen.push(lower);
            let label = wallet.label.clone().unwrap_or_else(|| "wallet".to_string());
            // An address this build cannot read is reported here rather than
            // refused at load time, which would take every command down.
            let Some(chain) = wallet.chain() else {
                warnings.push(format!(
                    "{label}: {} is not an address coins can read — it expects an Ethereum address (0x + 40 hex) or a Solana address (43-44 base58)",
                    crate::wallet::short_address(&wallet.address)
                ));
                continue;
            };
            let map = tokens.iter().find(|(c, _)| *c == chain).map(|(_, m)| m);
            let amounts = self.one_wallet(wallet, chain, &label, ids, map, warnings);
            out.push(WalletBalances {
                label,
                address: wallet.address.clone(),
                amounts,
            });
        }
        out
    }

    /// What one address holds, of the coins asked for: the chain's own currency,
    /// its stake if it has any, and each tracked token on it.
    fn one_wallet(
        &self,
        wallet: &crate::config::Wallet,
        chain: Chain,
        label: &str,
        ids: &[String],
        tokens: Option<&BTreeMap<String, String>>,
        warnings: &mut Vec<String>,
    ) -> BTreeMap<String, f64> {
        let addr = wallet.address.to_ascii_lowercase();
        let mut amounts: BTreeMap<String, f64> = BTreeMap::new();
        let rpc = Rpc::new(chain, wallet.rpc.as_deref());

        // The chain's own currency, which every address on it can hold.
        let native = chain.native_coin();
        if ids.iter().any(|i| i == native) {
            let key = format!("wallet-{}-{addr}-{native}", chain.name());
            match self.cached_balance(&key, || rpc.native_balance(&wallet.address)) {
                Ok(v) => *amounts.entry(native.to_string()).or_insert(0.0) += v,
                // `{:#}` so the cause is shown: the outermost context is
                // only "via <endpoint>", which says nothing on its own.
                Err(e) => warnings.push(format!("{label}: {e:#}")),
            }
            // Staked SOL sits in accounts of its own, which `getBalance`
            // does not count. Left out, an address that stakes reads low
            // and says nothing about it.
            if chain == Chain::Solana {
                let key = format!("stake-{addr}");
                match self.cached_balance(&key, || rpc.staked_balance(&wallet.address)) {
                    Ok(v) if v > 0.0 => {
                        *amounts.entry(native.to_string()).or_insert(0.0) += v
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warnings.push(format!("{label}: staked SOL not counted — {e:#}"))
                    }
                }
            }
        }

        for (id, token) in tokens.into_iter().flatten() {
            // Ethereum needs the token's own decimals; a Solana token
            // account reports an amount that is already scaled.
            let decimals = match chain {
                Chain::Ethereum => {
                    let dec_key = format!("decimals-{token}");
                    match self.cache.get_fresh::<u32>(&dec_key, META_TTL) {
                        Some(d) => d,
                        None => match rpc.token_decimals(token) {
                            Ok(d) => {
                                self.cache.put(&dec_key, &d);
                                d
                            }
                            Err(_) => 18,
                        },
                    }
                }
                Chain::Solana => 0,
            };
            let key = format!("wallet-{}-{addr}-{id}", chain.name());
            match self.cached_balance(&key, || {
                rpc.token_balance(&wallet.address, token, decimals)
            }) {
                Ok(v) if v > 0.0 => *amounts.entry(id.clone()).or_insert(0.0) += v,
                Ok(_) => {}
                Err(e) => warnings.push(format!("{label} / {id}: {e:#}")),
            }
        }
        amounts
    }

    fn cached_balance<F>(&self, key: &str, fetch: F) -> Result<f64>
    where
        F: FnOnce() -> Result<f64>,
    {
        if !self.force {
            if let Some(v) = self.cache.get_fresh::<f64>(key, WALLET_TTL) {
                return Ok(v);
            }
            if self.reach != Reach::Fresh {
                if let Some(hit) = self.cache.get::<f64>(key) {
                    return Ok(hit.value);
                }
            }
        }
        if self.reach == Reach::Cache {
            // No chain call from a first paint: an address with nothing cached
            // simply has no figure yet.
            return Ok(0.0);
        }
        match fetch() {
            Ok(v) => {
                self.cache.put(key, &v);
                Ok(v)
            }
            Err(e) => match self.cache.get::<f64>(key) {
                Some(hit) => Ok(hit.value),
                None => Err(e),
            },
        }
    }

    /// Token addresses for tracked coins on one chain, cached per coin. A
    /// single `/coins/list` request covers every coin, so this costs at most
    /// one call per chain per 30 days.
    fn platform_contracts(
        &self,
        chain: Chain,
        ids: &[String],
        warnings: &mut Vec<String>,
    ) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut missing = Vec::new();
        let key = |id: &str| format!("contract-{}-{id}", chain.name());
        for id in ids {
            // The chain's own currency is not a token on it.
            if id == chain.native_coin() {
                continue;
            }
            match self.cache.get_fresh::<Option<String>>(&key(id), META_TTL) {
                Some(Some(addr)) => {
                    out.insert(id.clone(), addr);
                }
                Some(None) => {}
                None => missing.push(id.clone()),
            }
        }
        let Some(platform) = chain.platform_key() else { return out };
        if !missing.is_empty() {
            match self.api.platform_contracts(platform, ids) {
                Ok(found) => {
                    for id in ids {
                        let addr = found.get(id).cloned();
                        self.cache.put(&key(id), &addr);
                        if let Some(a) = addr {
                            out.insert(id.clone(), a);
                        }
                    }
                }
                Err(e) => warnings.push(format!(
                    "could not look up {} token addresses: {e}",
                    chain.name()
                )),
            }
        }
        out
    }

    /// `coins __warm`: refresh the cache quietly for the next invocation.
    pub fn warm(&self) -> Result<()> {
        let ids = self.cfg.quoted_coins();
        let changes = self.cfg.market_columns();
        let markets = self.api.markets(&self.fetch_ids(&ids), &self.cfg.currency, &changes, true)?;
        self.cache.put(&self.markets_key(), &markets);
        // Balances too, so a run with wallets doesn't block on the chain.
        if !self.cfg.wallets.is_empty() {
            let mut ignored = Vec::new();
            let _ = self.wallet_amounts(&ids, &mut ignored);
        }
        // The month columns need a chart of their own, on the same ceiling the
        // screen uses.
        if let Some(range) = self.cfg.month_chart_range() {
            for id in self.cfg.coins.iter().take(MAX_INLINE_SERIES) {
                let key = format!("chart-{id}-{}-{}", self.cfg.currency, range.key());
                if let Ok(v) = self.api.market_chart(id, &self.cfg.currency, range.days()) {
                    self.cache.put(&key, &v);
                }
            }
        }
        // The 1-week series rides along with the prices request; other ranges
        // need one call per coin, which is exactly what a background warm is for.
        if self.cfg.range != Range::W1 {
            let count = self.cfg.coins.len().min(MAX_INLINE_SERIES);
            for i in 0..count {
                if let Some(id) = self.cfg.coins.get(i) {
                    let key = format!("chart-{id}-{}-{}", self.cfg.currency, self.cfg.range.key());
                    if let Ok(v) = self.api.market_chart(id, &self.cfg.currency, self.cfg.range.days())
                    {
                        self.cache.put(&key, &v);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Extends a series with the live quote when it is newer than the last point.
fn append_quote(series: &mut Series, market: &Market) {
    let Some(price) = market.current_price.filter(|p| p.is_finite() && *p > 0.0) else { return };
    let ts = market
        .last_updated
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis());
    let Some(ts) = ts else { return };
    match series.last_mut() {
        Some((last_t, _)) if ts > *last_t => series.push((ts, price)),
        // The 7-day sparkline is pinned to this very timestamp, and at that
        // instant the quote is the authoritative value.
        Some((last_t, last_v)) if ts == *last_t => *last_v = price,
        Some(_) => {}
        None => series.push((ts, price)),
    }
}

/// Currencies common enough to trust without asking CoinGecko. Doubles as the
/// completion list for `-c`.
pub const COMMON_CURRENCIES: &[&str] = &[
    "usd", "eur", "gbp", "chf", "jpy", "cny", "dkk", "sek", "nok", "isk", "pln", "czk", "huf",
    "aud", "cad", "nzd", "sgd", "hkd", "krw", "inr", "brl", "mxn", "zar", "try", "rub", "ils",
    "aed", "sar", "thb", "twd", "vnd", "php", "idr", "myr", "btc", "eth", "sats", "bits",
];

/// One row per coin to display, coloured by its place in that order.
/// The keyless allowance and the way out of it. One wording, wherever a request
/// could not be made, because the answer is the same every time.
const RATE_LIMIT_HINT: &str =
    "the keyless allowance is 5-15 requests a minute — wait a moment, or put a\n\
     free demo key in `api_key` (`coins config --edit`) for 100 a minute";

/// Why a screen has no rows on it.
///
/// Asking for a coin whose price did not arrive is not the same as tracking
/// nothing, and telling someone to add a coin they have just added is worse than
/// saying nothing at all. The status knows which happened, so it says so.
fn no_rows_message(display: &[String], status: Status) -> String {
    if display.is_empty() {
        return "nothing tracked yet — add a coin with `coins add bitcoin`".into();
    }
    let names = match display {
        [one] => format!("{one:?}"),
        many => many.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", "),
    };
    match status {
        Status::RateLimited => format!(
            "no price for {names} yet — CoinGecko is rate-limiting this machine\n{RATE_LIMIT_HINT}"
        ),
        Status::Offline => format!(
            "no price for {names} yet — could not reach CoinGecko\ncheck your connection, then try again"
        ),
        // The prices did arrive, and this coin was not among them.
        _ => format!("coingecko has no market data for {names}"),
    }
}

/// The market list: the `n` most valuable coins, then the coins you track that
/// did not make it, in their own place in the market.
///
/// Returns the ids and where the second group starts, if there is one — a coin
/// of yours at rank 174 is worth seeing next to the giants, but it is not one of
/// them, and the screen should not pretend the list simply continues.
fn market_list(markets: &[Market], tracked: &[String], n: usize) -> (Vec<String>, Option<usize>) {
    let ranked = ranked_by_value(markets, n);
    let mut rest: Vec<&Market> = markets
        .iter()
        .filter(|m| tracked.iter().any(|t| t == &m.id) && !ranked.iter().any(|r| r == &m.id))
        .collect();
    rest.sort_by_key(|m| m.market_cap_rank.unwrap_or(u32::MAX));
    if rest.is_empty() {
        return (ranked, None);
    }
    let at = ranked.len();
    let mut ids = ranked;
    ids.extend(rest.into_iter().map(|m| m.id.clone()));
    (ids, Some(at))
}

/// The coins in a set, most valuable first.
///
/// By market capitalisation — circulating supply times price — which is what
/// "most valuable" means for a coin. Not by price per unit, which says only how
/// finely a supply was divided: XRP at €1.16 is worth more than BNB at €592.
/// Coins the API has no capitalisation for come last rather than first, and ties
/// keep the order they arrived in.
fn ranked_by_value(markets: &[Market], count: usize) -> Vec<String> {
    let mut ranked: Vec<&Market> = markets.iter().collect();
    ranked.sort_by(|a, b| {
        let (x, y) = (
            b.market_cap.unwrap_or(f64::MIN),
            a.market_cap.unwrap_or(f64::MIN),
        );
        x.total_cmp(&y)
    });
    ranked.into_iter().take(count).map(|m| m.id.clone()).collect()
}

/// Percentage change across the last `days` of a chart. The window is measured
/// back from the chart's own last point rather than from the clock, because the
/// history lags the live quote by up to an hour.
fn change_over_days(series: &Series, days: i64) -> Option<f64> {
    let (last_at, last) = *series.last()?;
    let cutoff = last_at - days * 86_400_000;
    let (_, first) = *series.iter().find(|(at, _)| *at >= cutoff)?;
    (first.is_finite() && first > 0.0 && last.is_finite())
        .then(|| (last - first) / first * 100.0)
}

fn build_rows(
    display: &[String],
    tracked: &[String],
    markets: &[Market],
    amounts: &BTreeMap<String, f64>,
    focused: bool,
) -> Vec<Row> {
    display
        .iter()
        .filter_map(|id| {
            let market = markets.iter().find(|m| &m.id == id)?;
            // A coin's colour is its place in `coins`, not its place on this
            // screen. The two are the same in the views built out of the config,
            // but `coins top` is ordered by the market: there SOL sits seventh
            // and STRK far below, and both would take the palette's last slot
            // and come out the same colour. Colour follows the coin.
            let color = if focused {
                0
            } else {
                tracked
                    .iter()
                    .position(|c| c == id)
                    .or_else(|| display.iter().position(|c| c == id))
                    .unwrap_or(0)
            };
            Some(Row {
                market: market.clone(),
                color,
                series: None,
                amount: amounts.get(id).copied().unwrap_or(0.0),
                changes: BTreeMap::new(),
                week_fallback: false,
            })
        })
        .collect()
}

/// A source's holdings as (coin id, amount), most valuable first.
fn breakdown(rows: &[Row], amounts: &BTreeMap<String, f64>) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = amounts
        .iter()
        .filter(|(_, a)| **a > 0.0)
        .map(|(id, a)| (id.clone(), *a))
        .collect();
    out.sort_by(|a, b| {
        let value = |id: &str, amount: f64| {
            rows.iter()
                .find(|r| r.market.id == id)
                .and_then(|r| r.market.current_price)
                .unwrap_or(0.0)
                * amount
        };
        value(&b.0, b.1).total_cmp(&value(&a.0, a.1))
    });
    out
}

/// One wallet's balances, before they are merged into the coin rows.
struct WalletBalances {
    label: String,
    address: String,
    amounts: BTreeMap<String, f64>,
}

#[derive(Debug)]
pub enum Match {
    /// One coin matched exactly, or one clearly leads on market cap.
    One(SearchCoin),
    /// Several exact matches — the caller must pick an id.
    Many(Vec<SearchCoin>),
    /// Nothing matched exactly; these are the nearest names we saw.
    Unknown(Vec<SearchCoin>),
}

/// Refuses a typo, but names the coins that might have been meant.
pub fn no_match_message(query: &str, near: &[SearchCoin]) -> String {
    if near.is_empty() {
        return format!(
            "no coin matches {query:?} — check the spelling, or pass a CoinGecko id such as \"bitcoin\""
        );
    }
    format!(
        "no coin is called {query:?} — did you mean one of these?\n{}\n(add by id, e.g. `coins add {}`)",
        coin_list(near),
        near[0].id
    )
}

fn coin_list(candidates: &[SearchCoin]) -> String {
    candidates
        .iter()
        .map(|c| {
            let rank = c
                .market_cap_rank
                .map(|r| format!(", #{r}"))
                .unwrap_or_default();
            format!("  {} ({}{})", c.id, c.symbol.to_ascii_uppercase(), rank)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn ambiguous_message(query: &str, candidates: &[SearchCoin]) -> String {
    format!(
        "several coins match {query:?} — re-run with one of these ids:\n{}",
        coin_list(candidates)
    )
}

fn plan_chart(view: View, focused: bool, count: usize) -> (ChartPlan, Vec<usize>) {
    if count == 0 {
        return (ChartPlan::Off, Vec::new());
    }
    // Both the list and the holdings view are tables, not plots; only `price
    // plot` (and a named coin) draws a chart.
    if matches!(view, View::List | View::Balance | View::Top) {
        return (ChartPlan::Off, Vec::new());
    }
    if focused || count == 1 {
        return (ChartPlan::Single, vec![0]);
    }
    (ChartPlan::Facets, (0..count.min(MAX_FACETS)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn market(id: &str, cap: Option<f64>) -> Market {
        serde_json::from_value(serde_json::json!({
            "id": id, "symbol": id, "name": id, "market_cap": cap
        }))
        .unwrap()
    }

    fn ranked(id: &str, cap: f64, rank: u32) -> Market {
        serde_json::from_value(serde_json::json!({
            "id": id, "symbol": id, "name": id, "market_cap": cap, "market_cap_rank": rank
        }))
        .unwrap()
    }

    #[test]
    fn how_far_to_reach_depends_on_what_is_on_disk() {
        let m = |s| Some(Duration::from_secs(s));
        // Current: nothing to fetch either way.
        assert_eq!(reach_for(m(30), false, false), Reach::Gaps);
        // Stale but recent: draw it now, refresh behind — a display instead does
        // the refreshing itself, so it reaches for fresh data every tick.
        assert_eq!(reach_for(m(600), false, false), Reach::Gaps);
        assert_eq!(reach_for(m(600), false, true), Reach::Fresh);
        // Past the hour, or nothing at all: worth waiting for.
        assert_eq!(reach_for(m(3601), false, false), Reach::Fresh);
        assert_eq!(reach_for(None, false, false), Reach::Fresh);
        // `--refresh` means what it says, whatever the cache holds.
        assert_eq!(reach_for(m(30), true, false), Reach::Fresh);
    }

    #[test]
    fn a_coin_keeps_its_colour_whatever_the_screen_orders_by() {
        let markets = vec![
            ranked("bitcoin", 1e12, 1),
            ranked("ethereum", 5e11, 2),
            ranked("tether", 1e11, 3),
            ranked("solana", 5e10, 7),
            ranked("starknet", 1e8, 174),
        ];
        let tracked = vec![
            "ethereum".to_string(),
            "solana".to_string(),
            "starknet".to_string(),
        ];
        // The market list puts strangers between them and pushes starknet far
        // down; the palette slots must still be 0, 1, 2.
        let display: Vec<String> = markets.iter().map(|m| m.id.clone()).collect();
        let rows = build_rows(&display, &tracked, &markets, &BTreeMap::new(), false);
        let slot = |id: &str| rows.iter().find(|r| r.market.id == id).unwrap().color;
        assert_eq!((slot("ethereum"), slot("solana"), slot("starknet")), (0, 1, 2));
        // Which is the same as in the view built straight out of the config.
        let own = build_rows(&tracked, &tracked, &markets, &BTreeMap::new(), false);
        for row in &own {
            assert_eq!(row.color, slot(&row.market.id), "{} changed colour", row.market.id);
        }
    }

    #[test]
    fn the_market_list_reaches_past_the_top_for_coins_you_track() {
        let markets = vec![
            ranked("big", 1e12, 1),
            ranked("large", 5e11, 2),
            ranked("small", 1e8, 174),
            ranked("tiny", 1e7, 402),
        ];
        let tracked = vec!["small".to_string(), "tiny".to_string()];
        let (ids, at) = market_list(&markets, &tracked, 2);
        assert_eq!(ids, ["big", "large", "small", "tiny"]);
        assert_eq!(at, Some(2), "the break is where the ranked coins end");
    }

    #[test]
    fn a_tracked_coin_already_in_the_top_is_not_repeated() {
        let markets = vec![ranked("big", 1e12, 1), ranked("large", 5e11, 2)];
        let tracked = vec!["big".to_string()];
        let (ids, at) = market_list(&markets, &tracked, 2);
        assert_eq!(ids, ["big", "large"]);
        assert_eq!(at, None, "nothing was appended, so there is no break to draw");
    }

    #[test]
    fn the_market_list_is_ordered_by_what_a_coin_is_worth() {
        // Price per unit says nothing: the cheap coin here is the valuable one.
        let markets = vec![
            market("expensive-and-small", Some(1e9)),
            market("cheap-and-huge", Some(1e12)),
            market("unranked", None),
            market("middling", Some(5e10)),
        ];
        let order = ranked_by_value(&markets, 10);
        assert_eq!(order, ["cheap-and-huge", "middling", "expensive-and-small", "unranked"]);
        // A coin the API has no capitalisation for goes last, never first.
        assert_eq!(order.last().unwrap(), "unranked");
        // And the count is a ceiling, not a promise.
        assert_eq!(ranked_by_value(&markets, 2).len(), 2);
        assert_eq!(ranked_by_value(&markets, 99).len(), 4);
    }

    #[test]
    fn coins_of_equal_value_keep_the_order_they_arrived_in() {
        let markets = vec![
            market("first", Some(2e9)),
            market("second", Some(2e9)),
            market("third", Some(2e9)),
        ];
        assert_eq!(ranked_by_value(&markets, 3), ["first", "second", "third"]);
    }

    #[test]
    fn an_empty_screen_says_which_thing_went_wrong() {
        let one = vec!["bitcoin".to_string()];
        // The case that started this: rate-limited, asking for a coin whose
        // price is not in the cache. Telling someone to add a coin they have
        // just added is worse than saying nothing.
        let limited = no_rows_message(&one, Status::RateLimited);
        assert!(limited.contains("rate-limiting"), "{limited}");
        assert!(limited.contains("bitcoin"), "{limited}");
        assert!(!limited.contains("coins add"), "{limited}");

        let offline = no_rows_message(&one, Status::Offline);
        assert!(offline.contains("could not reach"), "{offline}");
        assert!(!offline.contains("coins add"), "{offline}");

        // Prices did arrive and this coin was not among them: a name to fix.
        let fresh = no_rows_message(&one, Status::Fresh);
        assert!(fresh.contains("no market data"), "{fresh}");

        // Nothing asked for at all is the only case that asks for a coin.
        let empty = no_rows_message(&[], Status::Fresh);
        assert!(empty.contains("coins add bitcoin"), "{empty}");
    }

    #[test]
    fn a_month_change_is_measured_from_the_chart_end() {
        let day = 86_400_000i64;
        // 120 daily points, ending at day 0, rising 1 per day from 100.
        let series: Series = (0..120).map(|i| (-(119 - i) * day, 100.0 + i as f64)).collect();
        // 90 days back is the point at index 29, worth 129.
        let m3 = change_over_days(&series, 90).unwrap();
        assert!((m3 - (219.0 - 129.0) / 129.0 * 100.0).abs() < 1e-9, "{m3}");
        // A window longer than the chart uses everything it has, rather than
        // reporting nothing at all.
        let all = change_over_days(&series, 365).unwrap();
        assert!((all - 119.0).abs() < 1e-9, "{all}");
    }

    #[test]
    fn a_flat_or_empty_chart_yields_no_change() {
        assert_eq!(change_over_days(&Vec::new(), 90), None);
        let zeros: Series = vec![(0, 0.0), (86_400_000, 1.0)];
        assert_eq!(change_over_days(&zeros, 1), None, "a zero start has no percentage");
    }

    #[test]
    fn the_table_views_draw_no_big_chart() {
        for view in [View::List, View::Balance] {
            let (plan, charted) = plan_chart(view, false, 5);
            assert_eq!(plan, ChartPlan::Off, "{view:?} should stay a table");
            assert!(charted.is_empty());
        }
    }

    #[test]
    fn plot_draws_one_chart_per_coin() {
        assert_eq!(plan_chart(View::Plot, false, 5).0, ChartPlan::Facets);
        // A single coin is a single chart.
        assert_eq!(plan_chart(View::Plot, false, 1).0, ChartPlan::Single);
    }

    #[test]
    fn focus_always_wins() {
        assert_eq!(plan_chart(View::Plot, true, 5).0, ChartPlan::Single);
    }

    #[test]
    fn plotted_series_are_capped() {
        assert_eq!(plan_chart(View::Plot, false, 20).1.len(), MAX_FACETS);
    }
}
