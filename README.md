<img src="docs/icon.png" width="52" align="right" alt="">

# coins

[![CI](https://github.com/msimkin/coins/actions/workflows/ci.yml/badge.svg)](https://github.com/msimkin/coins/actions/workflows/ci.yml)

Cryptocurrency prices, charts and holdings in your terminal. Prices come from
[CoinGecko](https://docs.coingecko.com), quoted directly in your own currency.

<img src="docs/market.png" width="568" alt="Three coins with their price and the change over 1h, 24h, 30 days, 6 months and a year">

One row per coin: the price, then a change column for every period you name.

`coins plot` draws the charts, as many to a row as the terminal fits:

<img src="docs/plot.png" width="701" alt="Four charts in a two-by-two grid: BTC, ETH, SOL and STRK over a month">

`coins eth` draws one of them alone:

<img src="docs/eth.png" width="631" alt="One full-width chart of ETH over a month">

Curves are braille dots, 2×4 to a character cell, so they stay smooth at any width.

## Install

Rust 1.86 or newer.

```sh
git clone https://github.com/msimkin/coins
cargo install --path coins
coins completions install   # tab completion, one line into ~/.zshrc or ~/.bashrc
```

The first run writes `~/.config/coins/config.toml` with every option commented.

## Use

```sh
coins                 # prices
coins plot            # the charts
coins eth             # one coin, full width
coins add sol         # track a coin, by ticker, name or CoinGecko id
coins add 0xd8dA…     # track an address; its balances become holdings
coins rm sol          # stop tracking a coin or an address
coins balance         # prices, what each address holds, and the portfolio
coins config --edit   # open the config in $EDITOR
```

Any unambiguous prefix works: `coins b` is `coins balance`, `coins p` is `coins
plot`. `-c eur` and `-r 1m` change the currency and the period for one run.

`add` takes only an exact match on id, ticker or name, so a typo cannot quietly
become a coin you did not mean:

```
$ coins add bitcon
coins: no coin is called "bitcon" — did you mean one of these?
  bitcone (CONE, #5384)
(add by id, e.g. `coins add bitcone`)
```

The 250 largest coins are built in, so `coins add btc` needs no request. Tab
completion offers the same list.

## Configure

Everything lives in `~/.config/coins/config.toml`:

```toml
coins       = ["bitcoin", "ethereum"]  # tracked coins; order fixes each coin's colour
currency    = "usd"                    # any CoinGecko vs_currency: usd, eur, gbp, btc, …
range       = "1w"                     # how much history a chart covers
columns     = ["1h", "24h", "7d"]      # 1h 24h 7d 14d 30d 3m 6m 200d 1y

inline_plot = false                    # add a sparkline to the right of every row
show_addresses = false                 # let plain `coins` show holdings too
balance     = "all"                    # `coins balance`: all | addresses
height      = 14                       # chart height, in terminal rows
thousands   = " "                      # digit grouping: " " | "," | "." | ""
max_decimals = 3                       # ceiling on price decimals
theme       = "dark"                   # dark | light
api_key     = ""                       # a CoinGecko demo key raises the rate limit

[holdings]                             # coins held somewhere with no address
bitcoin = 0.25

[[wallets]]                            # read-only on-chain balances
address = "0x…"
label   = "main"
```

Every price in a column shares one decimal count — as many as its neediest row
needs, up to `max_decimals` — so the decimal marks line up on their own.

`coins add` and `coins rm` edit the file in place, leaving your comments alone.
`coins config` never parses it, so a file the tool refuses to read still has a way
in.

## Holdings

Two sources, which add up: `[holdings]` in the config, for coins held somewhere
without an address, and `coins add <address>` for balances read from the chain. The
chain follows from the address — `0x` and 40 hex digits is Ethereum, 43–44 base58
characters is Solana — and one address holds the chain's own coin plus any token you
track, so the group has a row per holding rather than per address.

<img src="docs/balance.png" width="631" alt="The balance view: coins, then addresses with amounts, a total and an allocation bar">

The addresses and amounts above are made up.

**Plain `coins` shows prices only** — no addresses, no portfolio — so checking the
market with someone beside you does not put your holdings on the screen. `coins
balance` is the full picture. Two options move that line: `show_addresses = true`
puts holdings in every view, and `balance = "addresses"` leaves the coin table out
of `coins balance`, so the two commands divide the screen between them.

A token you hold but do not track stays invisible until you `coins add` it. Whichever
RPC endpoint answers sees the address you asked about; the defaults are public nodes,
and `rpc = "…"` on a wallet points at your own.

## Prices

The keyless CoinGecko API allows roughly 5–15 requests a minute; a free demo key in
`api_key` raises it to 100. At `range = "1w"` a screen is one request, which carries
every price, every change column and a week of history together. Other ranges need a
chart per coin, as do the `3m` and `6m` columns — one chart serves both, since 180
days of history contains the last 90.

Charts are cached in `~/.cache/coins` for five minutes to a day, depending on how
fast the period moves. The top line always states the age of what you are looking at,
and says `offline` or `rate-limited` when it could not refresh.

## Maintenance

`src/coins.rs` holds the built-in coin list. `coins __popular` prints a fresh one from
CoinGecko, ready to replace everything below that file's header.

## Licence

MIT — see [LICENSE](LICENSE).

Prices are fetched at runtime under CoinGecko's
[API terms](https://www.coingecko.com/en/api_terms); nothing from them is
redistributed here.
