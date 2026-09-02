//! `price` — cryptocurrency prices and charts in the terminal.
//!
//! Five commands, and one config file that holds every preference.

mod cache;
mod coingecko;
mod coins;
mod complete;
mod config;
mod data;
mod portfolio;
mod render;
mod wallet;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use config::Config;
use data::{Fetcher, Match, View};
use render::theme::{ColorLevel, Theme, term_width};

#[derive(Parser)]
#[command(
    version,
    about = "Cryptocurrency prices and charts in your terminal",
    infer_subcommands = true,
    after_help = "Every setting lives in one file: run `coins config --edit`.\n\
                  Any unambiguous prefix works: `coins b` is `coins balance`."
)]
struct Cli {
    /// Plot a single coin (ticker or CoinGecko id) — short for `coins plot COIN`
    coin: Option<String>,

    /// Quote in this currency, for this run only
    #[arg(short = 'c', long, value_name = "CODE", global = true)]
    currency: Option<String>,

    /// Timeframe for this run only: 1d, 1w, 1m, 3m, 6m, 1y, all
    #[arg(short = 'r', long, value_name = "RANGE", global = true)]
    range: Option<String>,

    /// Leave the plots out
    #[arg(long, global = true)]
    no_plot: bool,

    /// Ignore the cache and fetch now
    #[arg(long, global = true)]
    refresh: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Draw the full-size plots: one per coin, or a single named coin
    Plot {
        /// One coin, full size (ticker or CoinGecko id)
        #[arg(value_name = "COIN")]
        coin: Option<String>,
    },
    /// Track a coin, or an address whose balances count as holdings
    Add {
        #[arg(required = true, value_name = "COIN|ADDRESS")]
        what: Vec<String>,
        /// Name the address in the ADDRESSES group, instead of `wallet`
        #[arg(long, short = 'l', value_name = "NAME")]
        label: Option<String>,
    },
    /// Stop tracking a coin or an address
    Rm {
        #[arg(required = true, value_name = "COIN|ADDRESS")]
        what: Vec<String>,
    },
    /// The full picture: prices, what each address holds, and your portfolio
    Balance,
    /// The largest coins there are, by market capitalisation
    Top {
        /// How many to list, most valuable first
        #[arg(value_name = "N")]
        count: Option<usize>,
    },
    /// Print the config path, open it, or add options a new version introduced
    Config {
        /// Open the file in $VISUAL, $EDITOR, or vi
        #[arg(long)]
        edit: bool,
        /// Add any option missing from the file, with its comment and default
        #[arg(long)]
        sync: bool,
    },
    /// Shell completion: `coins completions install` wires it into your shell
    Completions {
        /// install | zsh | bash  (or a candidate feed used by the scripts)
        #[arg(default_value = "install")]
        what: String,
        /// Prefix being completed, or the shell name for `install`
        prefix: Option<String>,
        /// Bare candidates, without zsh's `name:description` form
        #[arg(long)]
        plain: bool,
    },
    /// Refresh the cache quietly; spawned in the background by a normal run
    #[command(name = "__warm", hide = true)]
    Warm,
    /// Print `src/coins.rs`'s table from CoinGecko, to regenerate it
    #[command(name = "__popular", hide = true)]
    Popular,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {e}", env!("CARGO_PKG_NAME"));
        for cause in e.chain().skip(1) {
            eprintln!("  {cause}");
        }
        // Most failures that survive to here are the config's, and the file is
        // one command away. Matched on the path rather than a word, so it holds
        // for $COINS_CONFIG too.
        if let Ok(path) = Config::path() {
            if format!("{e:#}").contains(&path.display().to_string()) {
                eprintln!("  fix it with `coins config --edit`");
            }
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    // Completion is asked for by a shell, often in a loop: answer it before
    // anything that could touch the network or fail on a broken config.
    if let Some(Command::Completions { what, prefix, plain }) = &cli.command {
        return complete::run(what, prefix.as_deref(), *plain);
    }
    // Before the config is parsed, because this is the command that repairs it.
    // A file `coins` refuses to read used to take `coins config --edit` down with
    // it, which left no way in but a text editor and a guess at the path.
    if let Some(Command::Config { edit, sync }) = &cli.command {
        return config_command(*edit, *sync);
    }
    let (mut cfg, created) = Config::load()?;
    let theme = Theme::new(cfg.theme, ColorLevel::detect());
    // Prices are the whole output, so how their digits group is set once here.
    render::fmt::set_thousands(&cfg.thousands);
    render::fmt::set_max_decimals(cfg.max_decimals);

    // Per-run overrides never touch the file.
    if let Some(c) = &cli.currency {
        cfg.currency = c.trim().to_ascii_lowercase();
    }
    if let Some(r) = &cli.range {
        cfg.range = config::Range::parse(r)?;
    }

    match &cli.command {
        Some(Command::Warm) => {
            let fetcher = Fetcher::new(cfg, true)?;
            // A background refresh must never be heard from.
            let _ = fetcher.warm();
            Ok(())
        }
        Some(Command::Popular) => popular(&cfg),
        Some(Command::Completions { .. }) => Ok(()), // handled above
        Some(Command::Plot { coin }) => {
            screen(cfg, &cli, &theme, View::Plot, coin.clone(), None)
        }
        Some(Command::Balance) => screen(cfg, &cli, &theme, View::Balance, None, None),
        // No count means the config's, which is what `top` in the file is for.
        Some(Command::Top { count }) => screen(cfg, &cli, &theme, View::Top, None, *count),
        Some(Command::Add { what, label }) => add(&cfg, what, label.as_deref(), &theme),
        Some(Command::Rm { what }) => remove(&cfg, what),
        Some(Command::Config { .. }) => Ok(()), // handled before the config is read
        None => {
            if created {
                let path = Config::path()?;
                eprintln!(
                    "{}",
                    theme.dim(&format!(
                        "wrote a starter config to {} — `coins config --edit` to change it",
                        path.display()
                    ))
                );
            }
            // A bare coin argument means "plot this one", the shorthand for
            // `coins plot COIN`; with no argument, the list is the screen.
            let view = if cli.coin.is_some() { View::Plot } else { View::List };
            let coin = cli.coin.clone();
            screen(cfg, &cli, &theme, view, coin, None)
        }
    }
}

fn screen(
    cfg: Config,
    cli: &Cli,
    theme: &Theme,
    view: View,
    coin: Option<String>,
    top_count: Option<usize>,
) -> Result<()> {
    // `coins top` needs nothing tracked: the market is the subject.
    if cfg.coins.is_empty() && coin.is_none() && view != View::Top {
        bail!("nothing tracked yet — add a coin with `coins add bitcoin`");
    }
    let inline_plots = cfg.inline_plot && !cli.no_plot;
    let fetcher = Fetcher::new(cfg, cli.refresh)?;
    fetcher.validate_currency()?;
    let snap = fetcher.snapshot(coin.as_deref(), view, inline_plots, top_count)?;
    for line in render::screen(&snap, &fetcher.cfg, theme, term_width()) {
        println!("{line}");
    }
    // A blank line, so the next prompt is not flush against the table.
    println!();
    Ok(())
}

/// `add` takes a coin or a wallet — an address is unmistakable by shape, so one
/// command covers both without a second one to remember.
fn add(cfg: &Config, what: &[String], label: Option<&str>, theme: &Theme) -> Result<()> {
    // A label names one address. Which of several it belonged to would be a
    // guess, and a wrong guess writes itself into the config.
    if label.is_some() && what.len() > 1 {
        bail!("`--label` names one address at a time");
    }
    if let (Some(l), [item]) = (label, what) {
        if !config::is_wallet_address(item) {
            bail!("`--label` names an address, and {item:?} is a coin — {l:?} has nothing to name");
        }
    }
    let fetcher = Fetcher::new(cfg.clone(), false)?;
    for item in what {
        if config::is_wallet_address(item) {
            let address = item.trim();
            let chain = config::Chain::detect(address).unwrap_or(config::Chain::Ethereum);
            if config::add_wallet(address, label)? {
                println!(
                    "added {} wallet {}{}",
                    chain.name(),
                    wallet::short_address(address),
                    label.map(|l| format!(" as {l:?}")).unwrap_or_default()
                );
                // The chain's own coin, without which the wallet would be read
                // and then have nowhere to appear.
                let native = chain.native_coin();
                if config::add_coin(native)? {
                    println!("also tracking {native}, which this wallet holds");
                }
                println!(
                    "{}",
                    theme.dim(
                        "tokens are read only for coins you track — `coins add` the ones you hold"
                    )
                );
            } else {
                println!("{} is already listed", wallet::short_address(address));
            }
            continue;
        }
        if item.starts_with("0x") || item.len() > 30 {
            bail!(
                "{item:?} looks like an address but matches no format price knows\n\
                 Ethereum: 0x + 40 hex characters\n\
                 Solana:   43-44 base58 characters (no 0, O, I or l)"
            );
        }
        // Resolve before writing, so a typo never lands in the config.
        match fetcher.search_best(item)? {
            Match::One(coin) => {
                if config::add_coin(&coin.id)? {
                    println!(
                        "added {} ({})",
                        coin.id,
                        coin.symbol.to_ascii_uppercase()
                    );
                } else {
                    println!("{} is already tracked", coin.id);
                }
            }
            Match::Many(candidates) => bail!("{}", data::ambiguous_message(item, &candidates)),
            Match::Unknown(near) => bail!("{}", data::no_match_message(item, &near)),
        }
    }
    Ok(())
}

fn remove(cfg: &Config, what: &[String]) -> Result<()> {
    for item in what {
        // Matched against the configured wallets first, so an address this
        // build can no longer parse can still be removed.
        let is_wallet = config::is_wallet_address(item)
            || cfg
                .wallets
                .iter()
                .any(|w| w.address.eq_ignore_ascii_case(item.trim()));
        if is_wallet {
            if config::remove_wallet(item)? {
                println!("removed wallet {}", wallet::short_address(item));
            } else {
                println!("{} was not listed", wallet::short_address(item));
            }
            continue;
        }
        // Removal should work offline: match what is in the config, by id or
        // by the ticker the table showed.
        let q = item.trim().to_ascii_lowercase();
        let id = if cfg.coins.contains(&q) {
            Some(q.clone())
        } else {
            ticker_to_id(cfg, &q)
        };
        match id {
            Some(id) if config::remove_coin(&id)? => println!("removed {id}"),
            _ => println!("{q} is not tracked"),
        }
    }
    Ok(())
}

/// Looks up a tracked coin by ticker using cached market data, so `coins rm btc`
/// works without a request.
fn ticker_to_id(cfg: &Config, query: &str) -> Option<String> {
    let cache = cache::Cache::new(config::cache_home().ok()?);
    let key_prefix = format!("markets-{}", cfg.currency);
    let dir = std::fs::read_dir(cache.dir()).ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&key_prefix) {
            continue;
        }
        let stem = name.trim_end_matches(".json");
        if let Some(hit) = cache.get::<Vec<coingecko::Market>>(stem) {
            if let Some(m) = hit
                .value
                .iter()
                .find(|m| m.symbol.eq_ignore_ascii_case(query) && cfg.coins.contains(&m.id))
            {
                return Some(m.id.clone());
            }
        }
    }
    None
}

/// `coins __popular`: the rows of `src/coins.rs`, freshly fetched.
///
/// Hidden, because it is for whoever maintains the tool rather than for whoever
/// runs it. The names are stripped of zero-width and control characters, which
/// count as one column and render as none — at least one listed coin ships them,
/// and they silently misalign every column to their right.
fn popular(cfg: &Config) -> Result<()> {
    let api = coingecko::Api::new(&cfg.api_key);
    let markets = api.top_markets("usd", 250, None)?;
    println!("/// (id, symbol, name), most valuable first.");
    println!("pub const POPULAR: &[(&str, &str, &str)] = &[");
    for m in &markets {
        println!(
            "    ({:?}, {:?}, {:?}),",
            m.id.trim(),
            m.symbol.to_ascii_lowercase(),
            render::fmt::clean_text(&m.name)
        );
    }
    println!("];");

    // CoinGecko's own categorisation rather than a guess at one. A pegged coin
    // can be told from its behaviour — a week without a move — but only while
    // the currency it is pegged to sits still against the one on the screen.
    let stable = api.top_markets("usd", 100, Some("stablecoins"))?;
    println!();
    println!("/// Coins pegged to a currency, which `coins top` shows in grey:");
    println!("/// they are dollars in another wrapper, not price action.");
    println!("pub const STABLECOINS: &[&str] = &[");
    for m in &stable {
        println!("    {:?},", m.id.trim());
    }
    println!("];");
    Ok(())
}

fn config_command(edit: bool, sync: bool) -> Result<()> {
    // Deliberately does not parse the file: this is the command you reach for
    // when it will not parse. Only its path, and a template if it is missing.
    let path = Config::path()?;
    if !path.exists() {
        Config::write_template(&path)?;
    }
    if sync {
        let added = config::sync()?;
        if added.is_empty() {
            println!("every option is already in {}", path.display());
        } else {
            println!("added to {}:", path.display());
            for key in added {
                println!("  {key}");
            }
        }
        return Ok(());
    }
    if !edit {
        println!("{}", path.display());
        return Ok(());
    }
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    // The editor may be a command line, e.g. `code --wait`.
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .with_context(|| format!("could not start {editor:?}"))?;
    if !status.success() {
        bail!("{editor} exited with {status}");
    }
    Ok(())
}
