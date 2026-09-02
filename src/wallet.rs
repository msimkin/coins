//! Read-only on-chain balances over plain JSON-RPC. No API key, no chain crate.
//!
//! Two dialects behind one type. Ethereum: `eth_getBalance` for ether, and an
//! `eth_call` of `balanceOf(address)` plus `decimals()` for an ERC-20. Solana:
//! `getBalance` for lamports, and `getTokenAccountsByOwner` filtered by mint for
//! an SPL token — that filter matters, because a single address can own
//! thousands of token accounts and almost all of them are spam.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use serde::Deserialize;

use crate::config::Chain;

/// Public endpoints, tried in order. Free ones come and go, so each chain has
/// more than one; `cloudflare-eth.com` was dropped after it started answering
/// every request with an internal error.
pub const ETHEREUM_RPCS: &[&str] = &[
    "https://ethereum-rpc.publicnode.com",
    "https://eth.drpc.org",
    "https://1rpc.io/eth",
];
pub const SOLANA_RPCS: &[&str] = &[
    "https://api.mainnet-beta.solana.com",
    "https://solana-rpc.publicnode.com",
];

/// Solana's stake program, and the shape of one of its accounts: 200 bytes, with
/// the withdraw authority — whoever can take the SOL back out — 44 bytes in.
const STAKE_PROGRAM: &str = "Stake11111111111111111111111111111111111111";
const STAKE_ACCOUNT_LEN: u64 = 200;
const STAKE_WITHDRAWER_AT: u64 = 44;

/// `balanceOf(address)` — the first four bytes of its keccak-256 hash.
const BALANCE_OF: &str = "0x70a08231";
/// `decimals()`, likewise. Asking the contract is free and authoritative.
const DECIMALS: &str = "0x313ce567";

#[derive(Debug, Deserialize)]
struct RpcResponse {
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    message: String,
}

pub struct Rpc {
    agent: ureq::Agent,
    urls: Vec<String>,
    chain: Chain,
}

impl Rpc {
    /// `preferred` is the per-wallet `rpc` setting; the public defaults for the
    /// chain act as fallbacks, because free endpoints come and go.
    pub fn new(chain: Chain, preferred: Option<&str>) -> Rpc {
        let mut urls: Vec<String> = Vec::new();
        if let Some(u) = preferred {
            let u = u.trim();
            if !u.is_empty() {
                urls.push(u.to_string());
            }
        }
        let defaults = match chain {
            Chain::Ethereum => ETHEREUM_RPCS,
            Chain::Solana => SOLANA_RPCS,
        };
        for d in defaults {
            if !urls.iter().any(|u| u == d) {
                urls.push((*d).to_string());
            }
        }
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            .build();
        Rpc { agent: config.into(), urls, chain }
    }

    /// Tries each endpoint in turn; reports the last failure if all are down.
    fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
        });
        let mut last: Option<anyhow::Error> = None;
        for url in &self.urls {
            match self.call_one(url, &body) {
                Ok(v) => return Ok(v),
                Err(e) => last = Some(e.context(format!("via {url}"))),
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("no {} RPC endpoint configured", self.chain.name())))
    }

    fn call_one(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let mut resp = self.agent.post(url).send_json(body)?;
        let parsed: RpcResponse = resp.body_mut().read_json()?;
        if let Some(e) = parsed.error {
            bail!("{}", e.message);
        }
        parsed.result.ok_or_else(|| anyhow!("empty RPC result"))
    }

    fn call_hex(&self, method: &str, params: serde_json::Value) -> Result<String> {
        match self.call(method, params)? {
            serde_json::Value::String(s) => Ok(s),
            other => bail!("unexpected RPC result {other}"),
        }
    }

    /// The chain's own currency: ether, or SOL.
    pub fn native_balance(&self, address: &str) -> Result<f64> {
        match self.chain {
            Chain::Ethereum => {
                let hex =
                    self.call_hex("eth_getBalance", serde_json::json!([address, "latest"]))?;
                Ok(hex_to_f64(&hex) / 1e18)
            }
            Chain::Solana => {
                let v = self.call("getBalance", serde_json::json!([address]))?;
                let lamports = v
                    .get("value")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| anyhow!("no balance in the response"))?;
                Ok(lamports / 1e9)
            }
        }
    }

    /// SOL held in stake accounts this address can withdraw from.
    ///
    /// `getBalance` counts only what is liquid, and staking is the ordinary
    /// thing to do with SOL — so without this a staker's holdings read low,
    /// quietly, which is the worst way for a number to be wrong.
    ///
    /// `dataSlice` asks for none of the account data: only the lamports each one
    /// holds are wanted, and endpoints are readier to answer that. Not all of
    /// them will answer at all — indexed queries are the first thing a free
    /// endpoint turns off — but the caller tries each in turn.
    pub fn staked_balance(&self, address: &str) -> Result<f64> {
        if self.chain != Chain::Solana {
            return Ok(0.0);
        }
        let accounts = self.call(
            "getProgramAccounts",
            serde_json::json!([
                STAKE_PROGRAM,
                {
                    "encoding": "base64",
                    "dataSlice": { "offset": 0, "length": 0 },
                    "filters": [
                        { "dataSize": STAKE_ACCOUNT_LEN },
                        { "memcmp": { "offset": STAKE_WITHDRAWER_AT, "bytes": address } }
                    ]
                }
            ]),
        )?;
        let lamports: f64 = accounts
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|a| a.get("account")?.get("lamports")?.as_f64())
                    .sum()
            })
            .unwrap_or(0.0);
        Ok(lamports / 1e9)
    }

    /// The token's own `decimals()`. Defaults to 18 for a contract that does
    /// not answer, which is the ERC-20 convention. Ethereum only — a Solana
    /// token account reports an already-scaled amount.
    pub fn token_decimals(&self, contract: &str) -> Result<u32> {
        let hex = self.call_hex(
            "eth_call",
            serde_json::json!([{ "to": contract, "data": DECIMALS }, "latest"]),
        )?;
        let v = hex_to_f64(&hex);
        Ok(if (0.0..=36.0).contains(&v) { v as u32 } else { 18 })
    }

    /// A token balance: an ERC-20 by contract, or an SPL token by mint.
    pub fn token_balance(&self, address: &str, token: &str, decimals: u32) -> Result<f64> {
        match self.chain {
            Chain::Ethereum => {
                let addr = address.trim_start_matches("0x").trim_start_matches("0X");
                let data = format!("{BALANCE_OF}{:0>64}", addr.to_ascii_lowercase());
                let hex = self.call_hex(
                    "eth_call",
                    serde_json::json!([{ "to": token, "data": data }, "latest"]),
                )?;
                // A non-token contract answers "0x"; empty means no balance.
                if hex.trim_start_matches("0x").is_empty() {
                    return Ok(0.0);
                }
                Ok(hex_to_f64(&hex) / 10f64.powi(decimals as i32))
            }
            Chain::Solana => {
                // Filtered by mint on purpose: unfiltered, one address returned
                // 2807 token accounts. An owner may hold several accounts for
                // the same mint, so the balances are summed.
                let v = self.call(
                    "getTokenAccountsByOwner",
                    serde_json::json!([
                        address,
                        { "mint": token },
                        { "encoding": "jsonParsed" }
                    ]),
                )?;
                let accounts = v.get("value").and_then(|v| v.as_array());
                let mut total = 0.0;
                for a in accounts.into_iter().flatten() {
                    // `uiAmountString` is already scaled by the mint's decimals,
                    // and is a string so large balances keep their precision.
                    if let Some(s) = a
                        .pointer("/account/data/parsed/info/tokenAmount/uiAmountString")
                        .and_then(|v| v.as_str())
                    {
                        total += s.parse::<f64>().unwrap_or(0.0);
                    }
                }
                Ok(total)
            }
        }
    }
}

/// Hex (up to a full uint256) to f64. Accumulating in f64 loses precision past
/// 2^53, which is far below anything a displayed balance depends on, and it
/// cannot overflow the way a u128 parse would.
fn hex_to_f64(hex: &str) -> f64 {
    let mut v = 0f64;
    for c in hex.trim().trim_start_matches("0x").trim_start_matches("0X").chars() {
        match c.to_digit(16) {
            Some(d) => v = v * 16.0 + d as f64,
            None => return v,
        }
    }
    v
}

/// `0x71C…4f2` — enough to recognise an address, short enough for a status line.
pub fn short_address(address: &str) -> String {
    if address.len() < 12 {
        return address.to_string();
    }
    format!("{}…{}", &address[..6], &address[address.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses() {
        assert_eq!(hex_to_f64("0x0"), 0.0);
        assert_eq!(hex_to_f64("0x10"), 16.0);
        // The balance the plan's verification step cross-checks.
        assert!((hex_to_f64("0x5d2659027b0b8043") / 1e18 - 6.712_150_161_831_46).abs() < 1e-9);
    }

    #[test]
    fn chains_are_told_apart_by_shape() {
        use crate::config::Chain;
        assert_eq!(
            Chain::detect("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
            Some(Chain::Ethereum)
        );
        assert_eq!(
            Chain::detect("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
            Some(Chain::Solana)
        );
        // Base58 excludes these, so they cannot be a Solana address.
        assert_eq!(Chain::detect("0OIl0OIl0OIl0OIl0OIl0OIl0OIl0OIl0OIl"), None);
        assert_eq!(Chain::detect("0xdeadbeef"), None);
        assert_eq!(Chain::detect("bitcoin"), None);
        assert_eq!(Chain::detect(""), None);
    }

    #[test]
    fn addresses_shorten() {
        assert_eq!(
            short_address("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
            "0xd8dA…6045"
        );
    }
}
