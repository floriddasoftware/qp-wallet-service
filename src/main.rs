use axum::{
    extract::Json,
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{post, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use serde_json::json;
use chrono::Local;

use qp_hd::qp44::{CoinType, Purpose, QP44, WalletRequest};
use qp_hd::purpose::SeedSource;

use k256::SecretKey;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use ed25519_dalek::SigningKey;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use tiny_keccak::{Keccak, Hasher};

// ── Request / Response shapes ──────────────────────────────────────────────

#[derive(Deserialize)]
struct BalanceRequest {
    address: String,
    coin: String,
}

#[derive(Serialize)]
struct BalanceResponse {
    address: String,
    coin: String,
    balance: f64,
    balance_usd: f64,
}

#[derive(Deserialize)]
struct DeriveRequest {
    user_id: String,
    coin: String, // "bitcoin" | "ethereum" | "tron" | "solana"
}

#[derive(Serialize)]
struct DeriveResponse {
    address: String,
    coin: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// ── Address derivation logic ───────────────────────────────────────────────

fn coin_from_str(s: &str) -> Option<CoinType> {
    match s.to_lowercase().as_str() {
        "bitcoin"  => Some(CoinType::Bitcoin),
        "ethereum" => Some(CoinType::Ethereum),
        "tron"     => Some(CoinType::Tron),
        "solana"   => Some(CoinType::Solana),
        _ => None,
    }
}

fn build_seed(user_id: &str) -> Vec<u8> {
    // Combine PROTOCOL_SEED env var with user_id to get a unique seed per user
    let protocol_seed = std::env::var("PROTOCOL_SEED")
        .expect("PROTOCOL_SEED must be set in .env");

    let combined = format!("{}{}", protocol_seed, user_id);

    // Hash to exactly 32 bytes
    let hash = Sha256::digest(combined.as_bytes());
    hash.to_vec()
}

fn derive_address(priv_bytes: &[u8], coin: CoinType) -> Result<String, String> {
    match coin {
        CoinType::Bitcoin => {
            let secret = SecretKey::from_slice(priv_bytes)
                .map_err(|e| format!("Invalid BTC key: {}", e))?;
            let public = secret.public_key();
            let pub_bytes = public.to_encoded_point(false);
            let sha = Sha256::digest(pub_bytes.as_bytes());
            let ripe = Ripemd160::digest(&sha);
            let mut payload = vec![0x00];
            payload.extend_from_slice(&ripe);
            let checksum = Sha256::digest(&Sha256::digest(&payload));
            payload.extend_from_slice(&checksum[..4]);
            Ok(bs58::encode(payload).into_string())
        }
        CoinType::Ethereum => {
            let secret = SecretKey::from_slice(priv_bytes)
                .map_err(|e| format!("Invalid ETH key: {}", e))?;
            let public = secret.public_key();
            let uncompressed = public.to_encoded_point(false);
            let mut keccak = Keccak::v256();
            let mut out = [0u8; 32];
            keccak.update(&uncompressed.as_bytes()[1..]);
            keccak.finalize(&mut out);
            Ok(format!("0x{}", hex::encode(&out[12..])))
        }
        CoinType::Tron => {
            let secret = SecretKey::from_slice(priv_bytes)
                .map_err(|e| format!("Invalid TRX key: {}", e))?;
            let public = secret.public_key();
            let uncompressed = public.to_encoded_point(false);
            let mut keccak = Keccak::v256();
            let mut out = [0u8; 32];
            keccak.update(&uncompressed.as_bytes()[1..]);
            keccak.finalize(&mut out);
            let mut payload = vec![0x41];
            payload.extend_from_slice(&out[12..]);
            let checksum = Sha256::digest(&Sha256::digest(&payload));
            payload.extend_from_slice(&checksum[..4]);
            Ok(bs58::encode(payload).into_string())
        }
        CoinType::Solana => {
            let bytes: [u8; 32] = priv_bytes
                .try_into()
                .map_err(|_| "Invalid SOL key length".to_string())?;
            let signing_key = SigningKey::from_bytes(&bytes);
            let public = signing_key.verifying_key();
            Ok(bs58::encode(public.to_bytes()).into_string())
        }
    }
}

// ── Route handler ──────────────────────────────────────────────────────────

async fn derive_wallet_handler(
    Json(payload): Json<DeriveRequest>,
) -> Result<ResponseJson<DeriveResponse>, (StatusCode, ResponseJson<ErrorResponse>)> {

    let coin_type = coin_from_str(&payload.coin).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            ResponseJson(ErrorResponse {
                error: format!("Unknown coin: {}", payload.coin),
            }),
        )
    })?;

    let seed_bytes = build_seed(&payload.user_id);

    let request = WalletRequest {
        seed: SeedSource::Raw(seed_bytes),
        purpose: Purpose::BIP44,
        coins: vec![coin_type],
        account: 0,
        index: 0,
    };

    let wallets = QP44::derive_wallet(request).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson(ErrorResponse {
                error: format!("Derivation failed: {}", e),
            }),
        )
    })?;

    let wallet = wallets.first().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson(ErrorResponse {
                error: "No wallet derived".to_string(),
            }),
        )
    })?;

    let address = derive_address(&wallet.coordinate, coin_type).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson(ErrorResponse { error: e },
        ))
    })?;

    Ok(ResponseJson(DeriveResponse {
        address,
        coin: payload.coin,
    }))
}

async fn balance_handler(
    Json(payload): Json<BalanceRequest>,
) -> Result<ResponseJson<BalanceResponse>, (StatusCode, ResponseJson<ErrorResponse>)> {

    let balance = match payload.coin.to_lowercase().as_str() {
        "tron" => fetch_tron_balance(&payload.address).await,
        "bitcoin" => fetch_bitcoin_balance(&payload.address).await,
        "ethereum" => fetch_ethereum_balance(&payload.address).await,
        "solana" => fetch_solana_balance(&payload.address).await,
        _ => Err(format!("Unknown coin: {}", payload.coin)),
    };

    match balance {
        Ok(bal) => Ok(ResponseJson(BalanceResponse {
            address: payload.address,
            coin: payload.coin,
            balance: bal,
            balance_usd: 0.0, // will wire up price feed later
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson(ErrorResponse { error: e }),
        )),
    }
}

async fn fetch_tron_balance(address: &str) -> Result<f64, String> {
    let url = format!(
        "https://apilist.tronscanapi.com/api/account/tokens?address={}&start=0&limit=20&hidden=0&show=0&sortType=0&sortBy=0",
        address
    );

    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .header("TRON-PRO-API-KEY", "")
        .send()
        .await
        .map_err(|e| format!("TronScan request failed: {}", e))?;

    let json: serde_json::Value = res.json().await
        .map_err(|e| format!("TronScan parse failed: {}", e))?;

    // Look for USDT TRC-20 token
    if let Some(data) = json["data"].as_array() {
        for token in data {
            let name = token["tokenName"].as_str().unwrap_or("");
            let abbr = token["tokenAbbr"].as_str().unwrap_or("");
            if name == "Tether USD" || abbr == "USDT" {
                let balance_str = token["quantity"].as_str().unwrap_or("0");
                let balance: f64 = balance_str.parse().unwrap_or(0.0);
                return Ok(balance);
            }
        }
    }

    Ok(0.0) // No USDT found = zero balance
}

async fn fetch_bitcoin_balance(address: &str) -> Result<f64, String> {
    let url = format!("https://blockstream.info/api/address/{}", address);
    let client = reqwest::Client::new();
    let res = client.get(&url).send().await
        .map_err(|e| format!("Bitcoin request failed: {}", e))?;
    let json: serde_json::Value = res.json().await
        .map_err(|e| format!("Bitcoin parse failed: {}", e))?;

    let funded = json["chain_stats"]["funded_txo_sum"].as_u64().unwrap_or(0);
    let spent  = json["chain_stats"]["spent_txo_sum"].as_u64().unwrap_or(0);
    let balance_satoshi = funded.saturating_sub(spent);
    Ok(balance_satoshi as f64 / 100_000_000.0) // Convert satoshi to BTC
}

async fn fetch_ethereum_balance(address: &str) -> Result<f64, String> {
    let url = format!(
        "https://api.etherscan.io/api?module=account&action=balance&address={}&tag=latest&apikey=YourApiKeyToken",
        address
    );
    let client = reqwest::Client::new();
    let res = client.get(&url).send().await
        .map_err(|e| format!("Ethereum request failed: {}", e))?;
    let json: serde_json::Value = res.json().await
        .map_err(|e| format!("Ethereum parse failed: {}", e))?;

    let wei_str = json["result"].as_str().unwrap_or("0");
    let wei: f64 = wei_str.parse().unwrap_or(0.0);
    Ok(wei / 1e18) // Convert wei to ETH
}

async fn fetch_solana_balance(address: &str) -> Result<f64, String> {
    let url = "https://api.mainnet-beta.solana.com";
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBalance",
        "params": [address]
    });

    let client = reqwest::Client::new();
    let res = client.post(url).json(&body).send().await
        .map_err(|e| format!("Solana request failed: {}", e))?;
    let json: serde_json::Value = res.json().await
        .map_err(|e| format!("Solana parse failed: {}", e))?;

    let lamports = json["result"]["value"].as_u64().unwrap_or(0);
    Ok(lamports as f64 / 1_000_000_000.0) // Convert lamports to SOL
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "success": true,
        "message": "Server is awake",
        "time": Local::now().format("%-m/%-d/%Y, %-I:%M:%S %p").to_string()
    }))
}

// ── Server startup ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/wallet/derive", post(derive_wallet_handler))
        .route("/wallet/balance", post(balance_handler))
        .route("/health", get(health_check))
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    println!("🚀 Wallet service running on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}