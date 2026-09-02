//! The top coins by market capitalisation, generated from CoinGecko.
//!
//! Two jobs: tab-completion candidates, and resolving a ticker to a coin id
//! without a network request. Regenerate with the snippet in README.md when it
//! grows stale — nothing breaks if it is out of date, since anything missing
//! still resolves through `/search`.
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
    ("uniswap", "uni", "Uniswap"),
    ("the-open-network", "gram", "Gram (prev. Toncoin)"),
    ("global-dollar", "usdg", "Global Dollar"),
    ("hedera-hashgraph", "hbar", "Hedera"),
    ("avalanche-2", "avax", "Avalanche"),
    ("shiba-inu", "shib", "Shiba Inu"),
    ("sui", "sui", "Sui"),
    ("paypal-usd", "pyusd", "PayPal USD"),
    ("blackrock-usd-institutional-digital-liquidity-fund", "buidl", "BlackRock USD Institutional Digital Liquidity Fund"),
    ("hashnote-usyc", "usyc", "Circle USYC"),
    ("tether-gold", "xaut", "Tether Gold"),
    ("crypto-com-chain", "cro", "Cronos"),
    ("near", "near", "NEAR Protocol"),
    ("ripple-usd", "rlusd", "Ripple USD"),
    ("memecore", "m", "MemeCore"),
    ("okb", "okb", "OKB"),
    ("ondo-us-dollar-yield", "usdy", "Ondo US Dollar Yield"),
    ("bittensor", "tao", "Bittensor"),
    ("aave", "aave", "Aave"),
    ("aster-2", "aster", "Aster"),
    ("pax-gold", "paxg", "PAX Gold"),
    ("mantle", "mnt", "Mantle"),
    ("world-liberty-financial", "wlfi", "World Liberty Financial"),
    ("morpho", "morpho", "Morpho"),
    ("pump-fun", "pump", "Pump.fun"),
    ("ondo-finance", "ondo", "Ondo"),
    ("sky", "sky", "Sky"),
    ("ethena", "ena", "Ethena"),
    ("usdd", "usdd", "USDD"),
    ("htx-dao", "htx", "HTX DAO"),
    ("polkadot", "dot", "Polkadot"),
    ("pepe", "pepe", "Pepe"),
    ("internet-computer", "icp", "Internet Computer"),
    ("falcon-finance", "usdf", "Falcon USD"),
    ("bitget-token", "bgb", "Bitget Token"),
    ("bfusd", "bfusd", "BFUSD"),
    ("worldcoin-wld", "wld", "Worldcoin"),
    ("united-stables", "u", "United Stables"),
    ("usdgo", "usdgo", "USDGO"),
    ("bitway", "btw", "Bitway"),
    ("spiko-amundi-overnight-swap-fund-eur", "eursafo", "Spiko Amundi Overnight Swap Fund (EUR)"),
    ("ethereum-classic", "etc", "Ethereum Classic"),
    ("pi-network", "pi", "Pi Network"),
    ("polygon-ecosystem-token", "pol", "POL (ex-MATIC)"),
    ("blockchain-capital", "bcap", "Blockchain Capital"),
    ("kucoin-shares", "kcs", "KuCoin"),
    ("quant-network", "qnt", "Quant"),
    ("lighter", "lit", "Lighter"),
    ("eutbl", "eutbl", "Spiko EU T-Bills Money Market Fund"),
    ("just", "jst", "JUST"),
    ("janus-henderson-anemoy-treasury-fund", "jtrsy", "Janus Henderson Anemoy Treasury Fund"),
    ("gatechain-token", "gt", "Gate"),
    ("superstate-short-duration-us-government-securities-fund-ustb", "ustb", "Invesco Short Duration US Government Securities Fund"),
    ("nexo", "nexo", "NEXO"),
    ("algorand", "algo", "Algorand"),
    ("cosmos", "atom", "Cosmos Hub"),
    ("venice-token", "vvv", "Venice Token"),
    ("kaspa", "kas", "Kaspa"),
    ("arbitrum", "arb", "Arbitrum"),
    ("stable-2", "stable", "Stable"),
    ("render-token", "render", "Render"),
    ("janus-henderson-anemoy-aaa-clo-fund", "jaaa", "Janus Henderson Anemoy AAA CLO Fund"),
    ("jupiter-exchange-solana", "jup", "Jupiter"),
    ("gho", "gho", "GHO"),
    ("filecoin", "fil", "Filecoin"),
    ("beldex", "bdx", "Beldex"),
    ("official-trump", "trump", "Official Trump"),
    ("pancakeswap-token", "cake", "PancakeSwap"),
    ("curve-dao-token", "crv", "Curve DAO"),
    ("flare-networks", "flr", "Flare"),
    ("vechain", "vet", "VeChain"),
    ("xdce-crowd-sale", "xdc", "XDC Network"),
    ("usual-usd", "usd0", "Usual USD"),
    ("ether-fi", "ethfi", "Ether.fi"),
    ("dash", "dash", "Dash"),
    ("pudgy-penguins", "pengu", "Pudgy Penguins"),
    ("spx6900", "spx", "SPX6900"),
    ("true-usd", "tusd", "TrueUSD"),
    ("usdtb", "usdtb", "USDtb"),
    ("injective-protocol", "inj", "Injective"),
    ("hash-2", "hash", "Provenance Blockchain"),
    ("blockstack", "stx", "Stacks"),
    ("bianrensheng", "币安人生", "币安人生 (BinanceLife)"),
    ("aptos", "apt", "Aptos"),
    ("euro-coin", "eurc", "EURC"),
    ("a7a5", "a7a5", "A7A5"),
    ("ylds", "ylds", "YLDS"),
    ("aerodrome-finance", "aero", "Aerodrome Finance"),
    ("virtual-protocol", "virtual", "Virtuals Protocol"),
    ("pyth-network", "pyth", "Pyth Network"),
    ("layerzero", "zro", "LayerZero"),
    ("fetch-ai", "fet", "Artificial Superintelligence Alliance"),
    ("celestia", "tia", "Celestia"),
    ("ousg", "ousg", "Ondo Short-Term U.S. Government Bond Fund"),
    ("first-digital-usd", "fdusd", "First Digital USD"),
    ("kinesis-gold", "kau", "Kinesis Gold"),
    ("falcon-finance-ff", "ff", "Falcon Finance"),
    ("kite-2", "kite", "Kite"),
    ("apxusd", "apxusd", "apxUSD"),
    ("bitcoin-cash-sv", "bsv", "Bitcoin SV"),
    ("sofiusd", "sofid", "SoFiUSD"),
    ("sei-network", "sei", "Sei"),
    ("sun-token", "sun", "Sun Token"),
    ("pendle", "pendle", "Pendle"),
    ("midnight-3", "night", "Midnight"),
    ("pieverse", "pieverse", "Pieverse"),
    ("monad", "mon", "Monad"),
    ("gnosis", "gno", "Gnosis"),
    ("lido-dao", "ldo", "Lido DAO"),
    ("olympus", "ohm", "Olympus"),
    ("unibase", "ub", "Unibase"),
    ("onyc", "onyc", "OnRe Tokenized Reinsurance"),
    ("bittorrent", "btt", "BitTorrent"),
    ("terra-luna", "lunc", "Terra Luna Classic"),
    ("bonk", "bonk", "Bonk"),
    ("agora-dollar", "ausd", "AUSD"),
    ("crvusd", "crvusd", "crvUSD"),
    ("pons", "pons", "Pons"),
    ("decred", "dcr", "Decred"),
    ("cash-cat", "cashcat", "Cash Cat"),
    ("usx", "usx", "USX"),
    ("conflux-token", "cfx", "Conflux"),
    ("apenft", "nft", "AINFT"),
    ("ape-and-pepe", "apepe", "Ape and Pepe"),
    ("reallink", "real", "RealLink"),
    ("jasmycoin", "jasmy", "JasmyCoin"),
    ("re-protocol-reusd", "reusd", "Re Protocol reUSD"),
    ("syrup", "syrup", "Maple Finance"),
    ("kinesis-silver", "kag", "Kinesis Silver"),
    ("tezos", "xtz", "Tezos"),
    ("usdai", "usdai", "USDai"),
    ("plasma", "xpl", "Plasma"),
    ("ethereum-name-service", "ens", "Ethereum Name Service"),
    ("floki", "floki", "FLOKI"),
    ("trust-wallet-token", "twt", "Trust Wallet"),
    ("grass", "grass", "Grass"),
    ("ribbita-by-virtuals", "tibbir", "Ribbita by Virtuals"),
    ("frax", "frax", "Legacy Frax Dollar"),
    ("convex-finance", "cvx", "Convex Finance"),
    ("optimism", "op", "Optimism"),
    ("jito-governance-token", "jto", "Jito"),
    ("bnb48-club-token", "koge", "KOGE"),
    ("raydium", "ray", "Raydium"),
    ("tradable-na-rent-financing-platform-sstn", "pc0000031", "Tradable NA Rent Financing Platform SSTN"),
    ("dogwifcoin", "wif", "dogwifhat"),
    ("akedo", "ake", "Akedo"),
    ("compound-governance-token", "comp", "Compound"),
    ("starknet", "strk", "Starknet"),
    ("doublezero", "2z", "DoubleZero"),
    ("usa", "usat", "USAT"),
    ("spiko-us-t-bills-money-market-fund", "ustbl", "Spiko US T-Bills Money Market Fund"),
    ("kaia", "kaia", "Kaia"),
    ("zebec-network", "zbcn", "Zebec Network"),
    ("iota", "iota", "IOTA"),
    ("societe-generale-forge-eurcv", "eurcv", "EUR CoinVertible"),
    ("apyusd", "apyusd", "apyUSD"),
    ("ultima", "ultima", "Ultima"),
    ("eigenlayer", "eigen", "EigenCloud (prev. EigenLayer)"),
    ("the-graph", "grt", "The Graph"),
    ("theta-token", "theta", "Theta Network"),
    ("coco-2", "coco", "coco"),
    ("swissborg", "borg", "SwissBorg"),
    ("telcoin", "tel", "Telcoin"),
    ("collector-crypt", "cards", "Collector Crypt"),
    ("tradable-apac-diversified-finance-provider-sstn", "pc0000033", "Tradable APAC Diversified Finance Provider SSTN"),
    ("fartcoin", "fartcoin", "Fartcoin"),
    ("mx-token", "mx", "MX"),
    ("axie-infinity", "axs", "Axie Infinity"),
    ("safo", "safo", "Spiko Amundi Overnight Swap Fund"),
    ("vision-3", "vsn", "Vision"),
    ("thorchain", "rune", "THORChain"),
    ("strategy-pp-variable-xstock", "strcx", "Strategy PP Variable xStock"),
    ("gusd", "gusd", "GUSD"),
    ("akash-network", "akt", "Akash Network"),
    ("humanity", "h", "Humanity"),
    ("seeker", "skr", "Seeker"),
    ("arweave", "ar", "Arweave"),
    ("decentraland", "mana", "Decentraland"),
    ("shuffle-2", "shfl", "Shuffle"),
    ("origintrail", "trac", "OriginTrail"),
    ("safecoin", "safe", "SAFEbit"),
    ("neo", "neo", "NEO"),
    ("derive", "drv", "Derive"),
    ("chiliz", "chz", "Chiliz"),
    ("gmt-token", "gomining", "GoMining Token"),
    ("ecash", "xec", "eCash"),
    ("apecoin", "ape", "ApeCoin"),
    ("tradable-latam-fintech-sstn", "pc0000097", "Tradable LatAm Fintech SSTN"),
    ("avant-usd", "avusd", "Avant USD"),
    ("btse-token", "btse", "BTSE Token"),
    ("edgex", "edge", "edgeX"),
    ("non-playable-coin", "npc", "Non-Playable Coin"),
    ("elrond-erd-2", "egld", "MultiversX"),
    ("rollbit-coin", "rlb", "Rollbit Coin"),
    ("build-on", "b", "BUILDon"),
    ("chain-2", "xcn", "Onyxcoin"),
    ("safepal", "sfp", "SafePal"),
    ("kamino", "kmno", "Kamino"),
    ("vaulta", "a", "Vaulta"),
    ("jpysc", "jpysc", "JPYSC"),
    ("ozone-chain", "ozo", "Ozone Chain"),
    ("unifai-network", "uai", "UnifAI Network"),
    ("cash-4", "cash", "CASH"),
    ("grx-chain", "grx", "GRX Chain"),
    ("1inch", "1inch", "1INCH"),
    ("meta-2-2", "meta", "MetaDAO"),
    ("havven", "snx", "Synthetix"),
    ("stp-network", "awe", "AWE Network"),
    ("theo-short-duration-us-treasury-fund", "thbill", "Theo Short Duration US Treasury Fund"),
    ("dog-go-to-the-moon-rune", "dog", "Dog (Bitcoin)"),
    ("antfun", "antfun", "AntFun"),
    ("tradable-singapore-fintech-ssl-2", "pc0000023", "Tradable Singapore Fintech SSL"),
    ("alpha-bulgaria-warrants", "alfw", "Alpha Bulgaria Warrants"),
    ("astherus-usdf", "usdf", "Aster USDF"),
    ("permacast", "pwt", "Permacast"),
    ("railgun", "rail", "Railgun"),
    ("frax-usd", "frxusd", "Frax USD"),
    ("the-sandbox", "sand", "The Sandbox"),
    ("zama", "zama", "Zama"),
    ("cap-4", "cap", "Cap"),
    ("circle-internet-group-ondo-tokenized-stock", "crclon", "Circle Internet Group (Ondo Tokenized Stock)"),
    ("zano", "zano", "Zano"),
    ("sosovalue", "soso", "SoSoValue"),
    ("immutable-x", "imx", "Immutable"),
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
