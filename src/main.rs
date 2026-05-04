use axum::{
    extract::Json,
    http::StatusCode,
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use axum::routing::{post, get};
use axum::Json;
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
        .route("/health", get(health_check))
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    println!("🚀 Wallet service running on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}