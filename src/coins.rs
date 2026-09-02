//! The top coins by market capitalisation, generated from CoinGecko.
//!
//! Two jobs: tab-completion candidates, and resolving a ticker to a coin id
//! without a network request. Nothing breaks if the list is out of date, since
//! anything missing still resolves through `/search`. To refresh it, replace
//! everything below with the output of `coins __popular`.
//!
//! Names are stripped of zero-width and control characters: they count as one
//! character but render as none, which silently misaligns every column to
//! their right. At least one listed coin ships them in its name.

/// (id, symbol, name), most valuable first.
pub const POPULAR: &[(&str, &str, &str)] = &[
    ("bitcoin", "btc", "Bitcoin"),
    ("ethereum", "eth", "Ethereum"),
    ("tether", "usdt", "Tether"),
    ("binancecoin", "bnb", "BNB"),
    ("ripple", "xrp", "XRP"),
    ("usd-coin", "usdc", "USDC"),
    ("solana", "sol", "Solana"),
    ("tron", "trx", "TRON"),
    ("figure-heloc", "figr_heloc", "Figure Heloc"),
    ("hyperliquid", "hype", "Hyperliquid"),
    ("zcash", "zec", "Zcash"),
    ("dogecoin", "doge", "Dogecoin"),
    ("rain", "rain", "Rain"),
    ("usds", "usds", "USDS"),
    ("monero", "xmr", "Monero"),
    ("leo-token", "leo", "LEO Token"),
    ("whitebit", "wbt", "WhiteBIT Coin"),
    ("chainlink", "link", "Chainlink"),
    ("cardano", "ada", "Cardano"),
    ("stellar", "xlm", "Stellar"),
    ("bitcoin-cash", "bch", "Bitcoin Cash"),
    ("dai", "dai", "Dai"),
    ("canton-network", "cc", "Canton"),
    ("ethena-usde", "usde", "Ethena USDe"),
    ("usd1-wlfi", "usd1", "USD1"),
    ("litecoin", "ltc", "Litecoin"),
    ("the-open-network", "gram", "Gram (prev. Toncoin)"),
    ("uniswap", "uni", "Uniswap"),
    ("hedera-hashgraph", "hbar", "Hedera"),
    ("global-dollar", "usdg", "Global Dollar"),
    ("avalanche-2", "avax", "Avalanche"),
    ("shiba-inu", "shib", "Shiba Inu"),
    ("sui", "sui", "Sui"),
    ("paypal-usd", "pyusd", "PayPal USD"),
    ("blackrock-usd-institutional-digital-liquidity-fund", "buidl", "BlackRock USD Institutional Digital Liquidity Fund"),
    ("hashnote-usyc", "usyc", "Circle USYC"),
    ("tether-gold", "xaut", "Tether Gold"),
    ("crypto-com-chain", "cro", "Cronos"),
    ("near", "near", "NEAR Protocol"),
    ("memecore", "m", "MemeCore"),
    ("ripple-usd", "rlusd", "Ripple USD"),
    ("okb", "okb", "OKB"),
    ("ondo-us-dollar-yield", "usdy", "Ondo US Dollar Yield"),
    ("bittensor", "tao", "Bittensor"),
    ("aster-2", "aster", "Aster"),
    ("aave", "aave", "Aave"),
    ("pax-gold", "paxg", "PAX Gold"),
    ("mantle", "mnt", "Mantle"),
    ("world-liberty-financial", "wlfi", "World Liberty Financial"),
    ("morpho", "morpho", "Morpho"),
    ("ondo-finance", "ondo", "Ondo"),
    ("pump-fun", "pump", "Pump.fun"),
    ("sky", "sky", "Sky"),
    ("usdd", "usdd", "USDD"),
    ("polkadot", "dot", "Polkadot"),
    ("ethena", "ena", "Ethena"),
    ("htx-dao", "htx", "HTX DAO"),
    ("bitway", "btw", "Bitway"),
    ("pepe", "pepe", "Pepe"),
    ("internet-computer", "icp", "Internet Computer"),
    ("bitget-token", "bgb", "Bitget Token"),
    ("falcon-finance", "usdf", "Falcon USD"),
    ("bfusd", "bfusd", "BFUSD"),
    ("worldcoin-wld", "wld", "Worldcoin"),
    ("united-stables", "u", "United Stables"),
    ("usdgo", "usdgo", "USDGO"),
    ("spiko-amundi-overnight-swap-fund-eur", "eursafo", "Spiko Amundi Overnight Swap Fund (EUR)"),
    ("ethereum-classic", "etc", "Ethereum Classic"),
    ("pi-network", "pi", "Pi Network"),
    ("polygon-ecosystem-token", "pol", "POL (ex-MATIC)"),
    ("blockchain-capital", "bcap", "Blockchain Capital"),
    ("kucoin-shares", "kcs", "KuCoin"),
    ("lighter", "lit", "Lighter"),
    ("quant-network", "qnt", "Quant"),
    ("just", "jst", "JUST"),
    ("eutbl", "eutbl", "Spiko EU T-Bills Money Market Fund"),
    ("gatechain-token", "gt", "Gate"),
    ("superstate-short-duration-us-government-securities-fund-ustb", "ustb", "Invesco Short Duration US Government Securities Fund"),
    ("nexo", "nexo", "NEXO"),
    ("algorand", "algo", "Algorand"),
    ("janus-henderson-anemoy-treasury-fund", "jtrsy", "Janus Henderson Anemoy Treasury Fund"),
    ("arbitrum", "arb", "Arbitrum"),
    ("cosmos", "atom", "Cosmos Hub"),
    ("kaspa", "kas", "Kaspa"),
    ("venice-token", "vvv", "Venice Token"),
    ("stable-2", "stable", "Stable"),
    ("render-token", "render", "Render"),
    ("jupiter-exchange-solana", "jup", "Jupiter"),
    ("janus-henderson-anemoy-aaa-clo-fund", "jaaa", "Janus Henderson Anemoy AAA CLO Fund"),
    ("gho", "gho", "GHO"),
    ("filecoin", "fil", "Filecoin"),
    ("beldex", "bdx", "Beldex"),
    ("flare-networks", "flr", "Flare"),
    ("pancakeswap-token", "cake", "PancakeSwap"),
    ("official-trump", "trump", "Official Trump"),
    ("vechain", "vet", "VeChain"),
    ("dash", "dash", "Dash"),
    ("curve-dao-token", "crv", "Curve DAO"),
    ("xdce-crowd-sale", "xdc", "XDC Network"),
    ("usual-usd", "usd0", "Usual USD"),
    ("ether-fi", "ethfi", "Ether.fi"),
    ("pudgy-penguins", "pengu", "Pudgy Penguins"),
    ("akedo", "ake", "Akedo"),
    ("aptos", "apt", "Aptos"),
    ("true-usd", "tusd", "TrueUSD"),
    ("spx6900", "spx", "SPX6900"),
    ("usdtb", "usdtb", "USDtb"),
    ("blockstack", "stx", "Stacks"),
    ("bianrensheng", "币安人生", "币安人生 (BinanceLife)"),
    ("injective-protocol", "inj", "Injective"),
    ("euro-coin", "eurc", "EURC"),
    ("pyth-network", "pyth", "Pyth Network"),
    ("aerodrome-finance", "aero", "Aerodrome Finance"),
    ("a7a5", "a7a5", "A7A5"),
    ("ylds", "ylds", "YLDS"),
    ("virtual-protocol", "virtual", "Virtuals Protocol"),
    ("hash-2", "hash", "Provenance Blockchain"),
    ("layerzero", "zro", "LayerZero"),
    ("fetch-ai", "fet", "Artificial Superintelligence Alliance"),
    ("ousg", "ousg", "Ondo Short-Term U.S. Government Bond Fund"),
    ("celestia", "tia", "Celestia"),
    ("kite-2", "kite", "Kite"),
    ("kinesis-gold", "kau", "Kinesis Gold"),
    ("first-digital-usd", "fdusd", "First Digital USD"),
    ("sei-network", "sei", "Sei"),
    ("falcon-finance-ff", "ff", "Falcon Finance"),
    ("apxusd", "apxusd", "apxUSD"),
    ("midnight-3", "night", "Midnight"),
    ("bitcoin-cash-sv", "bsv", "Bitcoin SV"),
    ("pendle", "pendle", "Pendle"),
    ("sofiusd", "sofid", "SoFiUSD"),
    ("sun-token", "sun", "Sun Token"),
    ("lido-dao", "ldo", "Lido DAO"),
    ("pieverse", "pieverse", "Pieverse"),
    ("monad", "mon", "Monad"),
    ("gnosis", "gno", "Gnosis"),
    ("pons", "pons", "Pons"),
    ("olympus", "ohm", "Olympus"),
    ("bittorrent", "btt", "BitTorrent"),
    ("unibase", "ub", "Unibase"),
    ("onyc", "onyc", "OnRe Tokenized Reinsurance"),
    ("terra-luna", "lunc", "Terra Luna Classic"),
    ("cash-cat", "cashcat", "Cash Cat"),
    ("bonk", "bonk", "Bonk"),
    ("crvusd", "crvusd", "crvUSD"),
    ("agora-dollar", "ausd", "AUSD"),
    ("conflux-token", "cfx", "Conflux"),
    ("usx", "usx", "USX"),
    ("decred", "dcr", "Decred"),
    ("ape-and-pepe", "apepe", "Ape and Pepe"),
    ("kinesis-silver", "kag", "Kinesis Silver"),
    ("apenft", "nft", "AINFT"),
    ("reallink", "real", "RealLink"),
    ("re-protocol-reusd", "reusd", "Re Protocol reUSD"),
    ("syrup", "syrup", "Maple Finance"),
    ("tezos", "xtz", "Tezos"),
    ("plasma", "xpl", "Plasma"),
    ("floki", "floki", "FLOKI"),
    ("usdai", "usdai", "USDai"),
    ("jasmycoin", "jasmy", "JasmyCoin"),
    ("trust-wallet-token", "twt", "Trust Wallet"),
    ("optimism", "op", "Optimism"),
    ("ethereum-name-service", "ens", "Ethereum Name Service"),
    ("grass", "grass", "Grass"),
    ("ribbita-by-virtuals", "tibbir", "Ribbita by Virtuals"),
    ("frax", "frax", "Legacy Frax Dollar"),
    ("raydium", "ray", "Raydium"),
    ("jito-governance-token", "jto", "Jito"),
    ("bnb48-club-token", "koge", "KOGE"),
    ("convex-finance", "cvx", "Convex Finance"),
    ("tradable-na-rent-financing-platform-sstn", "pc0000031", "Tradable NA Rent Financing Platform SSTN"),
    ("dogwifcoin", "wif", "dogwifhat"),
    ("starknet", "strk", "Starknet"),
    ("compound-governance-token", "comp", "Compound"),
    ("doublezero", "2z", "DoubleZero"),
    ("ultima", "ultima", "Ultima"),
    ("usa", "usat", "USAT"),
    ("kaia", "kaia", "Kaia"),
    ("eigenlayer", "eigen", "EigenCloud (prev. EigenLayer)"),
    ("apyusd", "apyusd", "apyUSD"),
    ("societe-generale-forge-eurcv", "eurcv", "EUR CoinVertible"),
    ("iota", "iota", "IOTA"),
    ("zebec-network", "zbcn", "Zebec Network"),
    ("the-graph", "grt", "The Graph"),
    ("swissborg", "borg", "SwissBorg"),
    ("spiko-us-t-bills-money-market-fund", "ustbl", "Spiko US T-Bills Money Market Fund"),
    ("telcoin", "tel", "Telcoin"),
    ("theta-token", "theta", "Theta Network"),
    ("coco-2", "coco", "coco"),
    ("seeker", "skr", "Seeker"),
    ("tradable-apac-diversified-finance-provider-sstn", "pc0000033", "Tradable APAC Diversified Finance Provider SSTN"),
    ("collector-crypt", "cards", "Collector Crypt"),
    ("mx-token", "mx", "MX"),
    ("safo", "safo", "Spiko Amundi Overnight Swap Fund"),
    ("axie-infinity", "axs", "Axie Infinity"),
    ("fartcoin", "fartcoin", "Fartcoin"),
    ("thorchain", "rune", "THORChain"),
    ("strategy-pp-variable-xstock", "strcx", "Strategy PP Variable xStock"),
    ("vision-3", "vsn", "Vision"),
    ("humanity", "h", "Humanity"),
    ("elrond-erd-2", "egld", "MultiversX"),
    ("arweave", "ar", "Arweave"),
    ("gusd", "gusd", "GUSD"),
    ("edgex", "edge", "edgeX"),
    ("akash-network", "akt", "Akash Network"),
    ("decentraland", "mana", "Decentraland"),
    ("shuffle-2", "shfl", "Shuffle"),
    ("safecoin", "safe", "SAFEbit"),
    ("origintrail", "trac", "OriginTrail"),
    ("derive", "drv", "Derive"),
    ("neo", "neo", "NEO"),
    ("chiliz", "chz", "Chiliz"),
    ("gmt-token", "gomining", "GoMining Token"),
    ("ecash", "xec", "eCash"),
    ("btse-token", "btse", "BTSE Token"),
    ("non-playable-coin", "npc", "Non-Playable Coin"),
    ("apecoin", "ape", "ApeCoin"),
    ("tradable-latam-fintech-sstn", "pc0000097", "Tradable LatAm Fintech SSTN"),
    ("avant-usd", "avusd", "Avant USD"),
    ("rollbit-coin", "rlb", "Rollbit Coin"),
    ("kamino", "kmno", "Kamino"),
    ("build-on", "b", "BUILDon"),
    ("chain-2", "xcn", "Onyxcoin"),
    ("vaulta", "a", "Vaulta"),
    ("jpysc", "jpysc", "JPYSC"),
    ("cash-4", "cash", "CASH"),
    ("ozone-chain", "ozo", "Ozone Chain"),
    ("safepal", "sfp", "SafePal"),
    ("grx-chain", "grx", "GRX Chain"),
    ("useless-3", "useless", "Useless Coin"),
    ("1inch", "1inch", "1INCH"),
    ("meta-2-2", "meta", "MetaDAO"),
    ("havven", "snx", "Synthetix"),
    ("stp-network", "awe", "AWE Network"),
    ("theo-short-duration-us-treasury-fund", "thbill", "Theo Short Duration US Treasury Fund"),
    ("dog-go-to-the-moon-rune", "dog", "Dog (Bitcoin)"),
    ("the-sandbox", "sand", "The Sandbox"),
    ("antfun", "antfun", "AntFun"),
    ("zama", "zama", "Zama"),
    ("tradable-singapore-fintech-ssl-2", "pc0000023", "Tradable Singapore Fintech SSL"),
    ("alpha-bulgaria-warrants", "alfw", "Alpha Bulgaria Warrants"),
    ("astherus-usdf", "usdf", "Aster USDF"),
    ("permacast", "pwt", "Permacast"),
    ("unifai-network", "uai", "UnifAI Network"),
    ("melania-meme", "melania", "Melania Meme"),
    ("circle-internet-group-ondo-tokenized-stock", "crclon", "Circle Internet Group (Ondo Tokenized Stock)"),
    ("frax-usd", "frxusd", "Frax USD"),
    ("railgun", "rail", "Railgun"),
    ("sosovalue", "soso", "SoSoValue"),
    ("cap-4", "cap", "Cap"),
];
/// One row of [`POPULAR`]: id, symbol, display name.
pub type Coin = (&'static str, &'static str, &'static str);

/// Resolves a query against the built-in list without touching the network.
///
/// An exact id always wins. A ticker resolves only when it is unique here —
/// `usdf` belongs to two coins in the top 250, and guessing between them is
/// exactly the mistake `/search` fuzzy-matching used to make.
pub fn resolve(query: &str) -> Option<&'static Coin> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return None;
    }
    if let Some(c) = POPULAR.iter().find(|c| c.0 == q) {
        return Some(c);
    }
    let mut by_symbol = POPULAR.iter().filter(|c| c.1 == q);
    match (by_symbol.next(), by_symbol.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// Completion candidates for a typed prefix, as (candidate, description).
///
/// Every candidate starts with `prefix` as typed. That is not cosmetic: bash
/// replaces the word with the candidates' longest common prefix, so offering
/// `bitcoin` for a typed `btc` shortens the line instead of completing it.
/// Both ids and tickers are valid `price add` arguments, so we offer whichever
/// actually matches.
pub fn candidates(prefix: &str) -> Vec<(&'static str, &'static str)> {
    let p = prefix.trim().to_ascii_lowercase();
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for c in POPULAR {
        if c.0.starts_with(&p) {
            out.push((c.0, c.2));
        } else if c.1.starts_with(&p) && !out.iter().any(|(cand, _)| *cand == c.1) {
            // The ticker matches but the id does not — offer the ticker, which
            // `price add` resolves just as well.
            out.push((c.1, c.2));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_unique_tickers_resolve_offline() {
        assert_eq!(resolve("bitcoin").unwrap().0, "bitcoin");
        assert_eq!(resolve("btc").unwrap().0, "bitcoin");
        assert_eq!(resolve("BTC").unwrap().0, "bitcoin");
        assert_eq!(resolve("eth").unwrap().0, "ethereum");
        assert!(resolve("definitely-not-a-coin").is_none());
        assert!(resolve("").is_none());
    }

    #[test]
    fn ambiguous_tickers_fall_through_to_search() {
        // Two coins in the top 250 use this ticker, so the list must not pick.
        let duplicated = POPULAR
            .iter()
            .find(|c| POPULAR.iter().filter(|o| o.1 == c.1).count() > 1)
            .map(|c| c.1);
        if let Some(sym) = duplicated {
            assert!(resolve(sym).is_none(), "{sym} is ambiguous and must not resolve");
        }
    }

    #[test]
    fn every_candidate_starts_with_what_was_typed() {
        for prefix in ["b", "bt", "btc", "bit", "usd", "sol", "eth", "e"] {
            let cands = candidates(prefix);
            assert!(!cands.is_empty(), "{prefix} should offer something");
            for (cand, _) in &cands {
                assert!(
                    cand.starts_with(prefix),
                    "{cand:?} does not start with {prefix:?} — bash would truncate the line"
                );
            }
        }
    }

    #[test]
    fn the_list_is_well_formed() {
        assert!(POPULAR.len() >= 200);
        for (id, sym, name) in POPULAR {
            assert!(!id.is_empty() && !sym.is_empty() && !name.is_empty());
            assert!(
                id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'),
                "{id} would need quoting in a shell"
            );
        }
    }
}
