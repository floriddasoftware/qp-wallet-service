use crate::economic_gate::BalanceProof;
use std::env;
use dotenvy::dotenv;

pub struct Config;

impl Config {
    /// 🔒 PRIVATE — cannot be accessed outside this module
    fn protocol_seed() -> [u8; 32] {
        dotenv().ok();

        let seed_hex =
            env::var("PROTOCOL_SEED")
                .expect("PROTOCOL_SEED not set in .env");

        let bytes = hex::decode(&seed_hex)
            .expect("Invalid PROTOCOL_SEED hex encoding (must be hex)");

        assert!(
            bytes.len() == 32,
            "PROTOCOL_SEED must be exactly 32 bytes (64 hex chars)"
        );

        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);

        seed
    }
}

/// 🔐 ONLY ENTRY POINT
pub(crate) fn require_seed(
    _proof: impl BalanceProof,
) -> [u8; 32] {
    Config::protocol_seed()
}