# coins

[![CI](https://github.com/msimkin/coins/actions/workflows/ci.yml/badge.svg)](https://github.com/msimkin/coins/actions/workflows/ci.yml)

Cryptocurrency prices, charts and holdings in your terminal. Type `coins`, see where
things stand.

Prices come from [CoinGecko](https://docs.coingecko.com), quoted directly in your own
currency. Balances, if you want them, are read straight from public Ethereum and
Solana endpoints — no API key, no account, and nothing sent anywhere but the coin ids
you track and the addresses you added yourself.

```
                                                             updated 16s ago
  ──────────────────────────────────────────────────────────────────────────
  COINS                  PRICE     1H    24H     30D  30 DAYS
  ● ETH   Ethereum  $2 390.467  ▼0.2%  ▼1.1%  ▲27.9%  ▁▁▁▂▂▁▁▁▁▁▁▃▆▇▇███▇██▇
  ● SOL   Solana       $99.602  ▼0.2%  ▼0.3%  ▲34.3%  ▁▁▁▁▂▂▂▂▁▁▂▂▄▅▆▆▆███▇▇
  ● STRK  Starknet      $0.027  ▼0.1%  ▲0.3%   ▲8.8%  ▄▅▄▄▃▂▁▁▁▂▁▂▅█▇▇▅▆▄▅▅▆
```

One row per coin: the price, a change column for every period you ask for, and a
sparkline over the period in `range` — whose header names it, so `30 DAYS` is never a
guess. The line at the top carries only the age of the numbers; the currency is on
every price, so a title would only repeat it.

**Every price in the column shares one decimal count** — two by default, or as many
as the neediest of them asks for, up to `max_decimals`. Here STRK asks for three, so
all three rows show three. Equal fraction widths mean the decimal marks line up on
their own, so coins four orders of magnitude apart stay comparable down the page.

Your holdings are deliberately absent: plain `coins` never puts them on the screen.
`coins balance` does — see [Holdings](#holdings).

`coins plot` when you want the big picture, with charts that widen to fill the
terminal:

```
                                                                      updated 16s ago
  ───────────────────────────────────────────────────────────────────────────────────
          ● ETH  $2 390.47  ▲28.1%                   ● SOL  $99.60  ▲35.0%

         ┤                   ⡀  ⣀ ⢠⣤⡀ ⡀⢀     $110.0 ┤                         ⣠⡀ ⡀
  $2 400 ┤                  ⢰⠻⢦⠟⠉⠛⠋ ⠹⠛⠙⠋⠳⣤          ┤                      ⢀ ⢰⠃⠳⠛⠹⠶⣄
         ┤                 ⢠⠏                $100.0 ┤                   ⢀⡀⣀⡞⠷⡞     ⠈⠛
  $2 200 ┤                 ⡏                  $90.0 ┤                  ⣰⠋⠙⠉
         ┤                ⢰⠃                        ┤                 ⡼⠁
  $2 000 ┤     ⡀⣀         ⣸                   $80.0 ┤      ⣀⡀ ⡀     ⢀⣸⠁
         ┤⡴⠶⠛⠛⠛⠙⠉⠳⠶⠳⠶⠶⠶⠶⠛⠛⠃                         ┤⠶⠶⠶⣤⠶⠛⠉⠙⠛⠙⠛⠛⠛⠛⠛⠋⠁
         └────────────────────────────────          └────────────────────────────────
          3 Aug                     2 Sep            3 Aug                     2 Sep
```

Each coin gets **its own axis**, in money. Coins worth $2,400 and $100 cannot share
a y-scale usefully, so they don't.

Charts are drawn with braille dots — 2×4 per character cell — so curves stay smooth
at any terminal width.

## Install

Rust 1.86 or newer, and nothing else — no OpenSSL, no C dependency, no API key.

```sh
git clone https://github.com/msimkin/coins
cargo install --path coins
coins completions install   # tab completion, one line into your shell rc
```

The first run writes `~/.config/coins/config.toml` with every option commented, then
prints bitcoin and ethereum until you `coins add` your own.

## Use

```sh
coins                 # the list, one row per coin, each with its own plot
coins plot            # the full-size plots, one per coin
coins plot btc        # one coin, full size, with its high and low for the period
coins btc             # shorthand for the above
coins add sol         # track a coin (by ticker, name or CoinGecko id)
coins add 0xd8dA…     # track an Ethereum address; its balances become holdings
coins rm sol          # stop tracking a coin or an address
coins balance         # what each address holds, and what it is worth
coins config --edit   # open the config in $EDITOR
```

**Any unambiguous prefix works**, so the names stay spelled out while the typing
stays short: `coins b` is `coins balance`, `coins p` is `coins plot`, `coins a sol`
is `coins add sol`. Only `c` and `co` are ambiguous, between `config` and
`completions`, so they are not inferred at all — use `con` and `com`.

`add` refuses anything it cannot resolve exactly, and suggests what you might have
meant, so a typo never ends up in your config:

```
$ coins add bitcon
coins: no coin is called "bitcon" — did you mean one of these?
  bitcone (CONE, #5384)
(add by id, e.g. `coins add bitcone`)
```

The 250 most valuable coins are built in, so `coins add btc` resolves with no
network request at all — useful on a plane, and it keeps a fuzzy search from ever
guessing at a popular ticker.

Flags override the config for one run without saving: `-c/--currency`,
`-r/--range`, `--no-plot`, `--refresh` (skip the cache). They work before or after
a subcommand:

```sh
coins -c eur -r 1m
coins plot btc -r 1y
```

## Tab completion

```sh
coins completions install        # appends one line to ~/.zshrc or ~/.bashrc
coins completions zsh            # or print the script and wire it up yourself
```

Then, in a new shell:

```
$ coins add sol⇥          solana
$ coins add bit⇥          bitcoin  bitcoin-cash  bittensor  bitget-token …
$ coins rm ⇥              bitcoin  ethereum          (only what you track)
$ coins -r ⇥              1d  1w  1m  3m  6m  1y  all
```

Both zsh and bash are supported. The zsh script bootstraps `compinit` if your
shell has not — a `.zshrc` without it leaves `compdef` undefined, and completion
that fails silently is worse than none.

## Configure

Everything lives in `~/.config/coins/config.toml`, created with comments on first
run:

```toml
coins     = ["bitcoin", "ethereum"]   # tracked coins; order fixes each coin's colour
currency  = "usd"                     # any CoinGecko vs_currency: usd, eur, dkk, gbp, btc, ...
range     = "1w"                      # 1d | 1w | 1m | 3m | 6m | 1y | all

inline_plot = true                    # the little plot beside each coin in the list
show_addresses = false                # let plain `coins` show holdings too;
                                      # `coins balance` always does
height      = 14                      # `coins plot` height, in terminal rows
thousands   = " "                     # digit grouping in prices: " " | "," | "." | ""
max_decimals = 3                      # most decimals a price may show; the whole
                                      # column shares the largest count it needs

columns   = ["1h", "24h", "7d"]       # change columns: 1h 24h 7d 14d 30d 200d 1y
theme     = "dark"                    # dark | light — matches your terminal background
api_key   = ""                        # optional free CoinGecko demo key -> 100 req/min

[holdings]                            # coins held somewhere off-chain
bitcoin = 0.25

[[wallets]]                           # read-only on-chain balances
address = "0x…"
label   = "main"
```

`coins add` and `coins rm` edit this file in place and leave your comments and
layout alone.

**`coins config` never parses the file.** It is the command you reach for when the
config will not load, so it only needs the path — a file `coins` refuses to read no
longer takes the way in with it.

**`coins config --sync`** adds any option a newer version introduced, with its
comment and default, so an option can never sit invisible behind a default you never
saw. New keys go in *above* the `[[wallets]]` tables, because a top-level key written
after one belongs to that table as far as TOML is concerned. Every option lives in
this one file, each with a note beside it — a test asserts that none can be added to
the code without appearing here.

**`thousands`** defaults to a space because `$2,372` reads as two-and-a-bit to
anyone who uses `,` as the decimal mark. Set it to `","` or `"."` or `""` to taste.

**`max_decimals`** is the ceiling on price decimals, and the whole column shares
whichever count its neediest row asks for: two down to €0.10, then one more per
leading zero. `max_decimals = 2` pins every price at two decimals; raise it if you
track a coin so cheap that three would round it to `0.000` — the table says so when
that happens rather than showing you a coin apparently worth nothing.

Change columns carry one decimal (`▲28.1%`). The second is noise on a 28% move, and
it costs a character in every column.

## Holdings

Two ways, and they add up:

- `[holdings]` in the config, for coins on an exchange or in cold storage. They form
  a group labelled `off-chain`.
- `coins add <address>` for an Ethereum or Solana address. Balances are read from
  public endpoints — no API key, and nothing but the address itself is ever sent.

The chain is worked out from the address:

| chain | shape | read via |
|---|---|---|
| Ethereum | `0x` + 40 hex | JSON-RPC `eth_getBalance`, and `balanceOf` per tracked ERC-20 |
| Solana | 43–44 base58 | JSON-RPC `getBalance`, and `getTokenAccountsByOwner` per tracked SPL mint |

An address holds the chain's own coin **plus any token you track** — one address
holding several coins is normal, not a glitch, which is why the group has one row per
holding rather than one per address.

Bitcoin is not read. An address is recognisable but not supported, and resolving an
extended key would mean deriving addresses over secp256k1 — cryptography this tool
deliberately does not carry. An address it cannot read is reported as a warning
rather than refused, because a config that will not load takes every command with it.

**Plain `coins` shows prices only.** It lists exactly what the `coins` setting names, with no
addresses, no portfolio and no wallet warnings — so glancing at the market with
someone beside you does not put your holdings on the screen. `coins balance` is the
full picture: every coin you track *or* hold, each address, and the portfolio. Set
`show_addresses = true` if you would rather have it all by default. On a terminal too narrow for the extra columns
the group is dropped rather than allowed to overflow — first in a compact form
(shorter labels, coarser amounts), then not at all.

Two groups, one grid. The leading columns — identity, name or wallet, money, and
every change column — are shared and sized together, so they line up between the
groups. What follows is each group's own: a sparkline for the coins, an amount and an
address for the holdings.

```
                                                             updated 16s ago
  ──────────────────────────────────────────────────────────────────────────
  COINS                     PRICE     1H    24H     30D  30 DAYS
  ● ETH      Ethereum  $2 390.467  ▼0.2%  ▼1.1%  ▲27.9%  ▁▁▂▂▁▁▁▁▁▁▆▇▇█████▇
  ● SOL      Solana       $99.602  ▼0.2%  ▼0.3%  ▲34.3%  ▁▁▁▂▂▂▁▁▁▂▄▅▅▆▆██▇▇
  ● STRK     Starknet      $0.027  ▼0.1%  ▲0.3%   ▲8.8%  ▄▅▄▄▃▂▁▂▂▁▄█▇▇▅▅▄▅▆

  ADDRESSES                 VALUE     1H    24H     30D  AMOUNT  ADDRESS
  ● ETH      hardware  $29 656.62  ▼0.2%  ▼1.1%  ▲27.9%   12.41  0x1234…5678
  ● ETH      savings    $4 931.77  ▼0.2%  ▼1.1%  ▲27.9%    2.06  0xAbCd…Ef01
  ● STRK     savings       $87.61  ▼0.1%  ▲0.3%   ▲8.8%   3 250  0xAbCd…Ef01
  ● SOL      solana    $11 776.92  ▼0.2%  ▼0.3%  ▲34.3%  118.24  So1111…1112
             total     $46 452.92  ▼0.2%  ▼0.9%  ▲29.5%

  ████████████████████████████ █████████ █
  ● ETH 74.46%  ● SOL 25.35%  ● STRK 0.19%
```

The addresses and amounts above are made up; the total and the allocation bar close
the group.

A few things worth knowing:

- **Only tracked coins are looked up.** A token you hold but don't track is
  invisible; `coins add` it and it appears. This is also what keeps a Solana address
  usable at all: asking one for *every* token account returned 2807 of them, nearly
  all spam, so balances are fetched one tracked coin at a time.
- **The total is the last row of the group**, labelled `total` in the same column as
  the wallet labels, with a value-weighted change for each period — weighted over the
  holdings that have a figure, so a coin missing one shrinks the basket rather than
  skewing it. It appears only when there is more than one holding to add up, and the
  allocation bar under it is never drawn without it.
- **Dust is hidden.** A holding worth less than a hundredth of your currency is
  dropped — airdrops and contract leftovers leave most real addresses carrying a few,
  and a row reading `0.0000 / €0.00` is noise.
- **The RPC endpoint sees which address you ask about.** Defaults rotate through
  `ethereum-rpc.publicnode.com`, `eth.drpc.org`, `1rpc.io/eth` for Ethereum and
  `api.mainnet-beta.solana.com`, `solana-rpc.publicnode.com` for Solana; set
  `rpc = "…"` on a wallet to use your own node.

## Where the prices come from

[CoinGecko](https://docs.coingecko.com), which is the only free source that quotes
directly in real currencies — `eur`, `dkk`, `gbp` and about sixty more — rather than
using a dollar stablecoin as a stand-in. It needs no account: the keyless public API
allows roughly 5–15 requests a minute, and a free demo key in `api_key` raises that
to 100.

The list at `range = "1w"` costs **one** request: `/coins/markets` returns every
coin's price, its 1h/24h/7d change *and* a 7-day sparkline together. Other ranges
need one history request per coin, cached for 5 minutes to a day depending on how
fast that range moves. Above ten tracked coins the plots fall back to the free
7-day series rather than spending a request each, and the column header says so.

Responses are cached in `~/.cache/coins`:

- Under a minute old — drawn straight from cache, no request at all.
- Older than that but under ten minutes — drawn from cache immediately, while a
  detached background refresh makes the next run current.
- Older still — fetched before drawing.

The header always states the age of what you are looking at, and says `offline` or
`rate-limited` when it could not refresh but had something cached to show.

## Environment

| Variable | Effect |
|---|---|
| `NO_COLOR` | disable colour (truecolor, 256-colour and 16-colour terminals are detected otherwise) |
| `COINS_CONFIG` | use a different config file |
| `COINS_CACHE` | use a different cache directory |
| `COINS_WIDTH` | assume this terminal width — useful when piping to a file or pager |
| `COINS_API_BASE` | point at a CoinGecko mirror or proxy |

## Maintenance

`src/coins.rs` is generated. To refresh the built-in list:

```sh
curl -s "https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd&order=market_cap_desc&per_page=250&page=1" \
| python3 -c '
import json, sys, unicodedata
d = json.load(sys.stdin)
def clean(t):
    # Zero-width and control characters count as one column but render as none,
    # which silently misaligns every column to their right.
    t = "".join(c for c in t if unicodedata.category(c) not in ("Cf","Cc","Cs","Co","Cn"))
    return " ".join(t.split())
for c in d:
    print(f"""    ("{c["id"]}", "{c["symbol"].lower()}", "{clean(c["name"])}"),""")
'
```

Paste the rows into `POPULAR`. Nothing breaks if the list is stale — anything
missing still resolves through `/search`.

## Ideas not built yet

`--json` output for a status line; price alerts; candlesticks from `/coins/{id}/ohlc`;
ENS names (`coins add mark.eth`); chains beyond Ethereum; cost basis and P&L.

## Licence

MIT — see [LICENSE](LICENSE).

Prices are fetched at runtime under CoinGecko's
[API terms](https://www.coingecko.com/en/api_terms); nothing from them is
redistributed here, and the built-in list of popular coins holds ids and tickers
only.
