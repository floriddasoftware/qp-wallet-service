use crate::qp44::{CoinType};
use serde_json::json;

pub struct ChainInfo {
    pub path: &'static str,
    pub rpc: &'static str,
}

pub struct PathsRegistry;

impl PathsRegistry {

    pub fn get(coin: CoinType) -> Result<ChainInfo, String> {
        match coin {

            CoinType::Bitcoin => Ok(ChainInfo {
                path: "m/44'/0'/0'/0/0",
                rpc: "https://blockstream.info/api",
            }),

            CoinType::Ethereum => Ok(ChainInfo {
                path: "m/44'/60'/0'/0/0",
                rpc: "https://rpc.ankr.com/eth",
            }),

            CoinType::Solana => Ok(ChainInfo {
                path: "m/44'/501'/0'/0'",
                rpc: "https://api.mainnet-beta.solana.com",
            }),

            CoinType::Tron => Ok(ChainInfo {
                path: "m/44'/195'/0'/0/0",
                rpc: "https://api.trongrid.io",
            }),
        }
    }

    pub async fn query_balance(
        coin: CoinType,
        address: &str,
    ) -> Result<u128, String> {

        let info = Self::get(coin)?;

        let client = reqwest::Client::new();

        match coin {

            // ─────────────────────────────
            // BITCOIN
            // ─────────────────────────────
            CoinType::Bitcoin => {

                let url = format!("{}/address/{}/utxo", info.rpc, address);

                let resp = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|e| e.to_string())?;

                let total = resp.as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|u| u["value"].as_u64().unwrap_or(0) as u128)
                    .sum();

                Ok(total)
            }

            // ─────────────────────────────
            // ETHEREUM
            // ─────────────────────────────
            CoinType::Ethereum => {

                let body = json!({
                    "jsonrpc":"2.0",
                    "method":"eth_getBalance",
                    "params":[address, "latest"],
                    "id":1
                });

                let resp = client
                    .post(info.rpc)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|e| e.to_string())?;

                let hex_balance =
                    resp["result"].as_str().unwrap_or("0x0");

                u128::from_str_radix(
                    hex_balance.trim_start_matches("0x"),
                    16,
                )
                .map_err(|e| e.to_string())
            }

            // ─────────────────────────────
            // SOLANA
            // ─────────────────────────────
            CoinType::Solana => {

                let body = json!({
                    "jsonrpc":"2.0",
                    "id":1,
                    "method":"getBalance",
                    "params":[address]
                });

                let resp = client
                    .post(info.rpc)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|e| e.to_string())?;

                Ok(resp["result"]["value"]
                    .as_u64()
                    .unwrap_or(0) as u128)
            }

            // ─────────────────────────────
            // TRON
            // ─────────────────────────────
            CoinType::Tron => {

                let body = json!({
                    "address": address,
                    "visible": true
                });

                let resp = client
                    .post(format!("{}/wallet/getaccount", info.rpc))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|e| e.to_string())?;

                Ok(resp["balance"].as_u64().unwrap_or(0) as u128)
            }
        }
    }
}