# coins

[![CI](https://github.com/msimkin/coins/actions/workflows/ci.yml/badge.svg)](https://github.com/msimkin/coins/actions/workflows/ci.yml)

Cryptocurrency prices, charts and holdings in your terminal.

![The coins table: three coins with price, change columns and a sparkline each](docs/market.png)

One row per coin: the price, a change column for each period you ask for, and a
sparkline over the period in `range`. Prices come from
[CoinGecko](https://docs.coingecko.com), quoted directly in your own currency.

`coins plot` draws the same data full size, one chart per coin:

![Two full-size charts side by side, ETH and SOL over a month](docs/plot.png)

Every coin gets its own axis, in money: coins worth $2,400 and $100 cannot share one
usefully. Curves are drawn with braille dots, 2×4 per character cell, so they stay
smooth at any terminal width.

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
coins plot            # the full-size charts
coins plot btc        # one coin, full size
coins btc             # short for the above
coins add sol         # track a coin, by ticker, name or CoinGecko id
coins add 0xd8dA…     # track an address; its balances become holdings
coins rm sol          # stop tracking a coin or an address
coins balance         # prices, what each address holds, and the portfolio
coins config --edit   # open the config in $EDITOR
```

Any unambiguous prefix works: `coins b` is `coins balance`, `coins p` is `coins
plot`. `-c eur` and `-r 1m` override the configured currency and period for one run.

`add` takes only an exact match on id, ticker or name, so a typo cannot quietly
become a coin you did not mean:

```
$ coins add bitcon
coins: no coin is called "bitcon" — did you mean one of these?
  bitcone (CONE, #5384)
(add by id, e.g. `coins add bitcone`)
```

The 250 largest coins are built in, so `coins add btc` resolves without a request.
Tab completion draws on the same list.

## Configure

Everything lives in `~/.config/coins/config.toml`:

```toml
coins       = ["bitcoin", "ethereum"]  # tracked coins; order fixes each coin's colour
currency    = "usd"                    # any CoinGecko vs_currency: usd, eur, gbp, btc, …
range       = "1w"                     # 1d | 1w | 1m | 3m | 6m | 1y | all
columns     = ["1h", "24h", "7d"]      # change columns: 1h 24h 7d 14d 30d 3m 6m 200d 1y

inline_plot = true                     # the sparkline beside each coin
show_addresses = false                 # let plain `coins` show holdings too
height      = 14                       # `coins plot` height, in terminal rows
thousands   = " "                      # digit grouping in prices: " " | "," | "." | ""
max_decimals = 3                       # ceiling on price decimals; a column shares
                                       # whichever count its neediest row needs
theme       = "dark"                   # dark | light — match your terminal background
api_key     = ""                       # a free CoinGecko demo key raises the rate limit

[holdings]                             # coins held somewhere with no address
bitcoin = 0.25

[[wallets]]                            # read-only on-chain balances
address = "0x…"
label   = "main"
```

`coins add` and `coins rm` edit the file in place, leaving your comments and layout
alone. `coins config` never parses it, so a file the tool refuses to read still has
a way in.

## Holdings

Two sources, which add up:

- `[holdings]` in the config, for coins held somewhere without an address.
- `coins add <address>`, for balances read from the chain. The chain follows from the
  address: `0x` and 40 hex digits is Ethereum, read with `eth_getBalance` and
  `balanceOf` per tracked ERC-20; 43–44 base58 characters is Solana, read with
  `getBalance` and `getTokenAccountsByOwner` per tracked SPL mint.

One address holds the chain's own coin plus any token you track, so the group has a
row per holding rather than per address:

![The balance view: coins, addresses with amounts, a total and an allocation bar](docs/balance.png)

The addresses and amounts above are made up.

**Plain `coins` shows prices only** — no addresses, no portfolio — so checking the
market with someone beside you does not put your holdings on the screen. `coins
balance` is the full picture; `show_addresses = true` makes it the default.

Two things worth knowing:

- **Only coins you track are looked up.** A token you hold but do not track stays
  invisible until you `coins add` it.
- **Whichever RPC endpoint answers sees the address you asked about.** The defaults
  are public nodes; `rpc = "…"` on a wallet points at your own.

## Prices and caching

The keyless CoinGecko API allows roughly 5–15 requests a minute, and a free demo key
in `api_key` raises that to 100.

At `range = "1w"` a whole screen is **one** request: `/coins/markets` carries every
price, the change columns and a 7-day sparkline together. Other ranges need one
chart per coin, as do the `3m` and `6m` columns — one chart covers both, since 180
days of history contains the last 90. Charts are cached for 5 minutes to a day,
depending on how fast the period moves.

Responses live in `~/.cache/coins`. Under a minute old, a run draws from cache and
makes no request; older, it draws from cache at once and refreshes in the background;
older still, it fetches first. The top line always states the age of what you are
looking at, and says `offline` or `rate-limited` when it could not refresh.

## Environment

| Variable | Effect |
|---|---|
| `NO_COLOR` | disable colour (truecolor, 256-colour and 16-colour terminals are detected otherwise) |
| `COINS_CONFIG` | use a different config file |
| `COINS_CACHE` | use a different cache directory |
| `COINS_WIDTH` | assume this terminal width, for piping to a file or pager |
| `COINS_API_BASE` | point at a CoinGecko mirror or proxy |

## Licence

MIT — see [LICENSE](LICENSE).

Prices are fetched at runtime under CoinGecko's
[API terms](https://www.coingecko.com/en/api_terms); nothing from them is
redistributed here.
