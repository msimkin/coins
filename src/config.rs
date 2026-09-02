//! Settings live in one hand-editable TOML file. `add`/`rm` mutate it through
//! `toml_edit` so the user's comments and formatting survive.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

const TEMPLATE: &str = r#"# coins — terminal crypto dashboard.
# Every option lives here, each with a note on what it does.
# `coins config --sync` adds any option a newer version introduced.

coins          = ["bitcoin", "ethereum"]   # tracked coins; order fixes each coin's colour
currency       = "usd"      # any CoinGecko vs_currency: usd, eur, dkk, gbp, btc, ...
range          = "1w"       # period the plots and the last change column cover:
                            # 1d | 1w | 1m | 3m | 6m | 1y | all
columns        = ["1h", "24h", "7d"]       # change columns, left to right, from
                            # 1h 24h 7d 14d 30d 200d 1y

inline_plot    = true       # the small price plot on the right of each coin's row
                            # in `price`; false leaves the rows as numbers only
show_addresses = false      # whether plain `price` also shows what you hold;
                            # `coins balance` shows it either way
height         = 14         # height of a `coins plot` chart, in terminal rows

thousands      = " "        # digit grouping in prices: " " | "," | "." | ""
max_decimals   = 3          # most decimals a price may show; every price in a
                            # column shares the largest count any of them needs
theme          = "dark"     # dark | light — match your terminal background
api_key        = ""         # optional free CoinGecko demo key, for 100 requests
                            # a minute instead of the keyless 5-15

# Coins you hold that are not on an address below — an exchange, or cold storage.
# [holdings]
# bitcoin = 0.25

# Read-only on-chain balances. `coins add <address>` writes one of these for you.
# Only coins you track are looked up, so `coins add` the tokens you hold too.
# [[wallets]]
# address = "0x0000000000000000000000000000000000000000"  # Ethereum or Solana
# label   = "main"          # shown in the ADDRESSES group instead of the address
# rpc     = "https://ethereum-rpc.publicnode.com"  # optional; the operator of
                            # whichever endpoint you use sees the address you ask about
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    D1,
    W1,
    M1,
    M3,
    M6,
    Y1,
    All,
}

impl Range {
    pub fn parse(s: &str) -> Result<Range> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "1d" | "24h" | "d" | "day" => Range::D1,
            "1w" | "7d" | "w" | "week" => Range::W1,
            "1m" | "30d" | "m" | "month" => Range::M1,
            "3m" | "90d" | "quarter" => Range::M3,
            "6m" | "180d" => Range::M6,
            "1y" | "365d" | "y" | "year" => Range::Y1,
            "all" | "max" => Range::All,
            other => bail!(
                "unknown range {other:?} — use one of 1d, 1w, 1m, 3m, 6m, 1y, all",
            ),
        })
    }

    /// The `days` parameter CoinGecko wants. Granularity follows automatically:
    /// 1 day -> 5-minutely, 2-90 -> hourly, >90 -> daily. We never pass
    /// `interval`, which is an enterprise-only parameter.
    pub fn days(self) -> &'static str {
        match self {
            Range::D1 => "1",
            Range::W1 => "7",
            Range::M1 => "30",
            Range::M3 => "90",
            Range::M6 => "180",
            Range::Y1 => "365",
            Range::All => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Range::D1 => "24 hours",
            Range::W1 => "7 days",
            Range::M1 => "30 days",
            Range::M3 => "3 months",
            Range::M6 => "6 months",
            Range::Y1 => "1 year",
            Range::All => "all time",
        }
    }

    /// Short form, in the same vocabulary as the change columns — those are
    /// named after CoinGecko's own periods (`1h`, `24h`, `7d`, `30d`, `1y`), so
    /// a month has to read `30D` here too rather than `1M`.
    pub fn short(self) -> &'static str {
        match self {
            Range::D1 => "24H",
            Range::W1 => "7D",
            Range::M1 => "30D",
            Range::M3 => "90D",
            Range::M6 => "180D",
            Range::Y1 => "1Y",
            Range::All => "ALL",
        }
    }

    /// Longer ranges move slowly, so their cached series can live longer.
    pub fn chart_ttl(self) -> Duration {
        Duration::from_secs(match self {
            Range::D1 => 5 * 60,
            Range::W1 => 30 * 60,
            Range::M1 => 2 * 3600,
            Range::M3 | Range::M6 => 6 * 3600,
            Range::Y1 | Range::All => 24 * 3600,
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            Range::D1 => "1d",
            Range::W1 => "1w",
            Range::M1 => "1m",
            Range::M3 => "3m",
            Range::M6 => "6m",
            Range::Y1 => "1y",
            Range::All => "all",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

/// Unknown keys are rejected rather than ignored: a setting typed after a
/// `[[wallets]]` header belongs to the wallet as far as TOML is concerned, and
/// silently dropping it is how a mistyped config looks like a broken tool.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wallet {
    pub address: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub rpc: Option<String>,
}

impl Wallet {
    /// `None` when the address matches no chain this build reads. Deliberately
    /// not an error: a config that cannot load takes every command with it,
    /// including the one that edits the config.
    pub fn chain(&self) -> Option<Chain> {
        Chain::detect(&self.address)
    }
}

/// The file as serde sees it — strings here, parsed into enums by [`Config::load`].
#[derive(Debug, Deserialize)]
struct Raw {
    #[serde(default = "default_coins")]
    coins: Vec<String>,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_range")]
    range: String,
    #[serde(default = "default_height")]
    height: usize,
    #[serde(default = "default_true")]
    inline_plot: bool,
    #[serde(default)]
    show_addresses: bool,
    #[serde(default = "default_thousands")]
    thousands: String,
    #[serde(default = "default_max_decimals")]
    max_decimals: usize,
    /// A `[[wallets]]` entry whose header was left commented out lands here as
    /// three stray top-level keys. Serde would ignore them and the wallet would
    /// simply never appear, so they are caught instead.
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    rpc: Option<String>,
    #[serde(default = "default_columns")]
    columns: Vec<String>,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    holdings: BTreeMap<String, f64>,
    #[serde(default)]
    wallets: Vec<Wallet>,
}

fn default_coins() -> Vec<String> {
    vec!["bitcoin".into(), "ethereum".into()]
}
fn default_currency() -> String {
    "usd".into()
}
fn default_range() -> String {
    "1w".into()
}
fn default_thousands() -> String {
    " ".into()
}
fn default_max_decimals() -> usize {
    3
}
fn default_height() -> usize {
    14
}
fn default_true() -> bool {
    true
}
fn default_columns() -> Vec<String> {
    vec!["1h".into(), "24h".into(), "7d".into()]
}
fn default_theme() -> String {
    "dark".into()
}

#[derive(Debug, Clone)]
pub struct Config {
    pub coins: Vec<String>,
    pub currency: String,
    pub range: Range,
    pub height: usize,
    pub inline_plot: bool,
    /// Whether plain `price` also shows what you hold. Off by default, so
    /// glancing at prices with someone beside you does not put your portfolio
    /// on the screen; `coins balance` shows it when you want it.
    pub show_addresses: bool,
    /// Digit-group separator for prices. A space by default: "," and "." each
    /// mean the decimal point to half the world, so "$2,372" gets misread.
    pub thousands: String,
    /// The most decimals a price may show. Every price in a column shares one
    /// count — the largest any of them needs — so they stay comparable.
    pub max_decimals: usize,
    pub columns: Vec<String>,
    pub theme: Theme,
    pub api_key: String,
    pub holdings: BTreeMap<String, f64>,
    pub wallets: Vec<Wallet>,
}

/// Change columns we know how to ask CoinGecko for.
pub const CHANGE_COLUMNS: &[&str] = &["1h", "24h", "7d", "14d", "30d", "200d", "1y"];

impl Config {
    pub fn path() -> Result<PathBuf> {
        if let Some(p) = std::env::var_os("COINS_CONFIG") {
            return Ok(PathBuf::from(p));
        }
        Ok(config_home()?.join(env!("CARGO_PKG_NAME")).join("config.toml"))
    }

    /// Writes the commented starter template, creating the directory if needed.
    pub fn write_template(path: &std::path::Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(path, TEMPLATE)
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Reads the config, writing the commented template first if it is missing.
    /// Returns the config and whether the template was just created.
    pub fn load() -> Result<(Config, bool)> {
        let path = Self::path()?;
        let mut created = false;
        if !path.exists() {
            Self::write_template(&path)?;
            created = true;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let raw: Raw = toml::from_str(&text)
            .with_context(|| format!("{} is not valid TOML", path.display()))?;

        for c in &raw.columns {
            let c = c.to_ascii_lowercase();
            if !CHANGE_COLUMNS.contains(&c.as_str()) {
                bail!(
                    "unknown column {c:?} in {} — pick from {}",
                    path.display(),
                    CHANGE_COLUMNS.join(", ")
                );
            }
        }
        if raw.address.is_some() || raw.label.is_some() || raw.rpc.is_some() {
            bail!(
                "{} has `address` (or `label`/`rpc`) as a top-level setting\n\
                 those belong to a wallet, so they need a `[[wallets]]` line above them:\n\n\
                 \x20   [[wallets]]\n\
                 \x20   address = \"0x…\"\n\
                 \x20   label   = \"main\"\n\n\
                 uncomment that header, or let `coins add 0x…` write the whole entry",
                path.display()
            );
        }
        if raw.thousands.chars().count() > 1 {
            bail!(
                "`thousands` in {} must be a single character or empty — try \" \", \",\", \".\" or \"\"",
                path.display()
            );
        }
        let cfg = Config {
            coins: raw
                .coins
                .iter()
                .map(|c| c.trim().to_ascii_lowercase())
                .filter(|c| !c.is_empty())
                .collect(),
            currency: raw.currency.trim().to_ascii_lowercase(),
            range: Range::parse(&raw.range)?,
            height: raw.height.clamp(4, 60),
            inline_plot: raw.inline_plot,
            show_addresses: raw.show_addresses,
            thousands: raw.thousands.clone(),
            max_decimals: raw.max_decimals.min(10),
            columns: raw.columns.iter().map(|c| c.to_ascii_lowercase()).collect(),
            theme: match raw.theme.trim().to_ascii_lowercase().as_str() {
                "dark" => Theme::Dark,
                "light" => Theme::Light,
                other => bail!("unknown theme {other:?} — use dark or light"),
            },
            api_key: raw.api_key.trim().to_string(),
            holdings: raw
                .holdings
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect(),
            wallets: raw.wallets,
        };
        Ok((cfg, created))
    }

    /// The change columns, in config order, that CoinGecko can supply.
    pub fn change_columns(&self) -> Vec<&str> {
        self.columns
            .iter()
            .map(|s| s.as_str())
            .filter(|c| CHANGE_COLUMNS.contains(c))
            .collect()
    }

    /// Every coin to price: the tracked ones, plus anything held.
    ///
    /// A wallet's own chain currency counts as held whether or not it is
    /// tracked. Without that, adding a Bitcoin address while tracking only
    /// ethereum and solana fetched nothing and said nothing — the balance was
    /// there, and the tool simply never asked for it.
    pub fn quoted_coins(&self) -> Vec<String> {
        let mut v = self.coins.clone();
        for id in self.holdings.keys() {
            if !v.contains(id) {
                v.push(id.clone());
            }
        }
        for wallet in &self.wallets {
            let Some(chain) = wallet.chain() else { continue };
            let native = chain.native_coin().to_string();
            if !v.contains(&native) {
                v.push(native);
            }
        }
        v
    }
}

/// The chains whose balances `price` can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Ethereum,
    Solana,
}

impl Chain {
    /// Which chain an address belongs to, from its shape alone.
    ///
    /// The lengths do the work. A Solana public key is 32 bytes, which is
    /// always 43 or 44 base58 characters; a looser range (this once allowed
    /// 32-44) swallows Bitcoin's 26-35-character addresses and files them as
    /// Solana wallets that then report zero.
    pub fn detect(address: &str) -> Option<Chain> {
        let a = address.trim();
        if a.len() == 42
            && (a.starts_with("0x") || a.starts_with("0X"))
            && a[2..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return Some(Chain::Ethereum);
        }
        if (43..=44).contains(&a.len()) && is_base58(a) {
            return Some(Chain::Solana);
        }
        None
    }

    /// The coin id of the chain's own currency, which an address always holds.
    pub fn native_coin(self) -> &'static str {
        match self {
            Chain::Ethereum => "ethereum",
            Chain::Solana => "solana",
        }
    }

    /// The key this chain goes by in CoinGecko's `platforms` map.
    pub fn platform_key(self) -> Option<&'static str> {
        match self {
            Chain::Ethereum => Some("ethereum"),
            Chain::Solana => Some("solana"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Chain::Ethereum => "ethereum",
            Chain::Solana => "solana",
        }
    }
}

pub fn is_wallet_address(s: &str) -> bool {
    Chain::detect(s).is_some()
}

fn is_base58(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l'))
}

pub fn cache_home() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("COINS_CACHE") {
        return Ok(PathBuf::from(p));
    }
    if let Some(x) = std::env::var_os("XDG_CACHE_HOME") {
        if !x.is_empty() {
            return Ok(PathBuf::from(x).join(env!("CARGO_PKG_NAME")));
        }
    }
    Ok(home()?.join(".cache").join(env!("CARGO_PKG_NAME")))
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("$HOME is not set — set $COINS_CONFIG to a path instead"))
}

fn config_home() -> Result<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        if !x.is_empty() {
            return Ok(PathBuf::from(x));
        }
    }
    Ok(home()?.join(".config"))
}

/// Adds any option missing from an existing config, comment and default
/// included, and reports which. An option a newer version introduced is
/// otherwise invisible: it works from its default and never appears in the
/// file, so there is nothing to discover or edit.
pub fn sync() -> Result<Vec<String>> {
    let path = Config::path()?;
    if !path.exists() {
        Config::write_template(&path)?;
        return Ok(vec!["(wrote a new config)".to_string()]);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;

    // Walk the template, taking each top-level option with the comment lines
    // that belong to it.
    let lines: Vec<&str> = TEMPLATE.lines().collect();
    let mut added = Vec::new();
    let mut block: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(key) = top_level_key(lines[i]) else {
            i += 1;
            continue;
        };
        let mut own = vec![lines[i].to_string()];
        let mut j = i + 1;
        // Continuation comments are indented, so they cannot be confused with
        // the comment that introduces the next section.
        while j < lines.len() && lines[j].starts_with(' ') && lines[j].trim_start().starts_with('#')
        {
            own.push(lines[j].to_string());
            j += 1;
        }
        if doc.get(&key).is_none() {
            added.push(key);
            block.extend(own);
        }
        i = j;
    }
    if block.is_empty() {
        return Ok(Vec::new());
    }

    // Inserted *before* the first table, never appended: a top-level key
    // written after `[[wallets]]` belongs to that table as far as TOML is
    // concerned, which is exactly how an address once went unread.
    let mut out: Vec<String> = Vec::new();
    let mut placed = false;
    for line in text.lines() {
        if !placed && line.starts_with('[') {
            out.push("# added by `coins config --sync`".to_string());
            out.extend(block.iter().cloned());
            out.push(String::new());
            placed = true;
        }
        out.push(line.to_string());
    }
    if !placed {
        if !out.last().is_some_and(|l| l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push("# added by `coins config --sync`".to_string());
        out.extend(block);
    }
    std::fs::write(&path, out.join("\n") + "\n")
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(added)
}

/// The option name on a top-level `key = value` line, if that is what it is.
fn top_level_key(line: &str) -> Option<String> {
    if line.starts_with(' ') || line.starts_with('#') || line.starts_with('[') {
        return None;
    }
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .then(|| key.to_string())
}

// ---------------------------------------------------------------- mutation ---
// Edits go through toml_edit so comments and layout in the user's file survive.

fn read_doc() -> Result<(PathBuf, toml_edit::DocumentMut)> {
    let path = Config::path()?;
    let text = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        TEMPLATE.to_string()
    };
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    Ok((path, doc))
}

fn write_doc(path: &PathBuf, doc: &toml_edit::DocumentMut) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Appends a coin id to `coins`. Returns false if it was already there.
pub fn add_coin(id: &str) -> Result<bool> {
    let (path, mut doc) = read_doc()?;
    let arr = doc
        .entry("coins")
        .or_insert(toml_edit::value(toml_edit::Array::new()))
        .as_array_mut()
        .ok_or_else(|| anyhow!("`coins` in {} is not an array", path.display()))?;
    if arr.iter().any(|v| v.as_str() == Some(id)) {
        return Ok(false);
    }
    arr.push(id);
    // Keep the list readable: one line unless it has grown long.
    if arr.len() > 6 {
        arr.iter_mut().for_each(|v| {
            v.decor_mut().set_prefix("\n    ");
        });
        arr.set_trailing("\n");
        arr.set_trailing_comma(true);
    }
    write_doc(&path, &doc)?;
    Ok(true)
}

pub fn remove_coin(id: &str) -> Result<bool> {
    let (path, mut doc) = read_doc()?;
    let Some(arr) = doc.get_mut("coins").and_then(|c| c.as_array_mut()) else {
        return Ok(false);
    };
    let before = arr.len();
    arr.retain(|v| v.as_str() != Some(id));
    if arr.len() == before {
        return Ok(false);
    }
    write_doc(&path, &doc)?;
    Ok(true)
}

pub fn add_wallet(address: &str, label: Option<&str>) -> Result<bool> {
    let (path, mut doc) = read_doc()?;
    let tables = doc
        .entry("wallets")
        .or_insert(toml_edit::Item::ArrayOfTables(
            toml_edit::ArrayOfTables::new(),
        ))
        .as_array_of_tables_mut()
        .ok_or_else(|| anyhow!("`wallets` in {} is not a [[wallets]] list", path.display()))?;
    if tables.iter().any(|t| {
        t.get("address")
            .and_then(|a| a.as_str())
            .is_some_and(|a| a.eq_ignore_ascii_case(address))
    }) {
        return Ok(false);
    }
    let mut t = toml_edit::Table::new();
    t["address"] = toml_edit::value(address);
    if let Some(l) = label {
        t["label"] = toml_edit::value(l);
    }
    tables.push(t);
    write_doc(&path, &doc)?;
    Ok(true)
}

pub fn remove_wallet(address: &str) -> Result<bool> {
    let (path, mut doc) = read_doc()?;
    let Some(tables) = doc.get_mut("wallets").and_then(|w| w.as_array_of_tables_mut()) else {
        return Ok(false);
    };
    let before = tables.len();
    tables.retain(|t| {
        !t.get("address")
            .and_then(|a| a.as_str())
            .is_some_and(|a| a.eq_ignore_ascii_case(address))
    });
    if tables.len() == before {
        return Ok(false);
    }
    write_doc(&path, &doc)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bitcoin_address_is_never_taken_for_solana() {
        // 34 base58 characters is a Bitcoin address; the old 32-44 range
        // accepted it as a Solana wallet that then reported zero.
        assert_eq!(Chain::detect("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"), None);
    }

    #[test]
    fn chains_are_told_apart_by_shape() {
        let cases = [
            ("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045", Some(Chain::Ethereum)),
            ("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", Some(Chain::Solana)),
            // A Bitcoin address is not read by this build, but it must not be
            // mistaken for Solana either.
            ("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", None),
            ("3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy", None),
            ("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4", None),
            ("0xdeadbeef", None),
            ("bitcoin", None),
            ("", None),
        ];
        for (address, want) in cases {
            assert_eq!(Chain::detect(address), want, "{address}");
        }
    }

}

#[cfg(test)]
mod template_tests {
    use super::*;

    /// Every option the loader accepts must appear in the template with a note
    /// beside it, or it is an option nobody can discover.
    #[test]
    fn the_template_names_the_binary() {
        // The header greets the user with the command they typed; a rename that
        // forgot the template would leave it greeting them with the old name.
        assert!(TEMPLATE.contains(env!("CARGO_PKG_NAME")));
    }

    #[test]
    fn the_template_documents_every_option() {
        let options = [
            "coins",
            "currency",
            "range",
            "columns",
            "inline_plot",
            "show_addresses",
            "height",
            "thousands",
            "max_decimals",
            "theme",
            "api_key",
        ];
        let lines: Vec<&str> = TEMPLATE.lines().collect();
        for option in options {
            let at = lines
                .iter()
                .position(|l| top_level_key(l).as_deref() == Some(option))
                .unwrap_or_else(|| panic!("{option} is missing from the template"));
            // Its own line carries a comment, or the indented line below does.
            let documented = lines[at].contains('#')
                || lines
                    .get(at + 1)
                    .is_some_and(|l| l.starts_with(' ') && l.trim_start().starts_with('#'));
            assert!(documented, "{option} has no description in the template");
        }
        // The optional sections are shown too, commented out as examples.
        assert!(TEMPLATE.contains("# [holdings]"));
        assert!(TEMPLATE.contains("# [[wallets]]"));
    }

    #[test]
    fn the_template_is_valid_and_loads_with_its_own_defaults() {
        let raw: Raw = toml::from_str(TEMPLATE).expect("the template must parse");
        assert_eq!(raw.thousands, " ");
        assert_eq!(raw.max_decimals, 3);
        assert!(!raw.show_addresses);
        assert!(raw.inline_plot);
    }

    #[test]
    fn a_top_level_key_is_told_from_a_table_or_a_comment() {
        assert_eq!(top_level_key("coins = [\"btc\"]").as_deref(), Some("coins"));
        assert_eq!(top_level_key("# coins = 1"), None);
        assert_eq!(top_level_key("[[wallets]]"), None);
        // Indented lines belong to whatever came before them.
        assert_eq!(top_level_key("   address = \"0x\""), None);
    }
}
