#![allow(unused_imports)]
#![allow(mismatched_lifetime_syntaxes)]
use quantom_value::{QuantPerm, Dimension, TransitionHeritage};
use crate::purpose::{Purpose as SeedPurpose, SeedSource};
use crate::protocolvalue::Qtm; // ✅ delegate forensic truth
pub const PURPOSE_44: u128 = 44;
pub const HARDENED_OFFSET: u128 = 0x8000_0000;

// ─────────────────────────────────────────────
// 🔹 Coin Types
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoinType {
    Bitcoin  = 0,
    Ethereum = 60,
    Tron     = 195,
    Solana   = 501,
}

impl CoinType {
    pub fn retained_mass(self) -> u128 {
        self as u128
    }

    pub fn name(self) -> &'static str {
        match self {
            CoinType::Bitcoin => "Bitcoin",
            CoinType::Ethereum => "Ethereum",
            CoinType::Tron => "Tron",
            CoinType::Solana => "Solana",
        }
    }

    pub fn all() -> &'static [CoinType] {
        &[
            CoinType::Bitcoin,
            CoinType::Ethereum,
            CoinType::Solana,
            CoinType::Tron,
        ]
    }
}

// ─────────────────────────────────────────────
// 🔹 Purpose
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum Purpose {
    BIP44,
    BIP32,
    Custom(u32),
}

impl Purpose {
    pub fn value(self) -> u128 {
        match self {
            Purpose::BIP44 => 44,
            Purpose::BIP32 => 32,
            Purpose::Custom(v) => v as u128,
        }
    }
}

// ─────────────────────────────────────────────
// 🔹 Wallet Output (Public API)
// ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WalletOutput {
    pub coin: CoinType,
    pub coordinate: [u8; 32],
    pub commitment: [u8; 32],
}

// ─────────────────────────────────────────────
// 🔹 Internal HD State Output (FOR FORENSICS)
// ─────────────────────────────────────────────

#[repr(C)]
pub struct Heritage<'a> {
    pub state: &'a QuantPerm,
    pub transition: TransitionHeritage,
}

// ─────────────────────────────────────────────
// 🔹 Stateful Wallet Engine
// ─────────────────────────────────────────────

pub struct QP44Wallet {
    manifold: QuantPerm,
    external: u32,
    coin: CoinType,
}

impl QP44Wallet {
    pub fn from_quantperm(
        mut manifold: QuantPerm,
        coin: CoinType,
        external: u32,
    ) -> Self {
        manifold.set_initial_dimension_from_perm();


        Self { manifold, coin, external }
    }

    fn realize(&mut self, change: u32) -> Heritage {
        let total_mass =
            (PURPOSE_44 + HARDENED_OFFSET) +
            (self.coin.retained_mass() + HARDENED_OFFSET) +
            (0 + HARDENED_OFFSET) +
            (change as u128) +
            (self.external as u128);


        let qp = crate::protocol_id::QuantumId.quantum_seed(
            crate::economic_gate::verify_balance(1, 1)
                .expect("Economic gate failed"),
        );
    
        let sigma = self.manifold.structural_value();
        let before_dim = self.manifold.dimension();

        let gravity = transition(&mut self.manifold, &qp);
        let after_dim = self.manifold.dimension();
        self.manifold.retain(total_mass);
        
    // 🔥 Delegate forensic signals to QuantPerm::calculate_work
    let (tau, delta, gross_work) =
        QuantPerm::calculate_work(
            self.manifold.retained_mass(),
            &qp,          // use the protocol seed bytes as mirror
            after_dim,
            before_dim,
        );

    let net_work = gross_work.saturating_sub(sigma);

    Heritage {
        state: &self.manifold,

        transition: TransitionHeritage {
            tau,
            delta,
            gross_work,
            net_work,
            origin: gravity.origin,
        },
    }
    }

    pub fn next_receive(&mut self) -> Heritage {
        self.realize(0)
    }

    pub fn next_change(&mut self) -> Heritage {
        self.realize(1)
    }

    pub fn replay(&mut self, index: u32) {
        self.external = index;
    }

    pub fn into_manifold(self) -> QuantPerm {
        self.manifold
    }
}


//Transition
fn transition(
    manifold: &mut QuantPerm,
    seed: &[u8; 32],
) -> TransitionHeritage {
    manifold.transition(Some(seed))
}

// ─────────────────────────────────────────────
// 🔹 Wallet Request (SDK)
// ─────────────────────────────────────────────

pub struct WalletRequest {
    pub seed: SeedSource,
    pub purpose: Purpose,
    pub coins: Vec<CoinType>,
    pub account: u32,
    pub index: u32,
}

// ─────────────────────────────────────────────
// 🔹 QP44 SDK Engine
// ─────────────────────────────────────────────

pub struct QP44;

impl QP44 {
    pub fn derive_wallet(
        request: WalletRequest,
    ) -> Result<Vec<WalletOutput>, String> {
        let mut outputs = Vec::new();

        for coin in &request.coins {
            // 🔹 Base manifold from seed
            let base = SeedPurpose::quantperm_from_seed(request.seed.clone())?;

            // 🔹 Stateful driver
            let mut wallet = QP44Wallet::from_quantperm(base, *coin, request.index);

            // 🔹 Perform transition (THIS produces real state)
            let result = wallet.next_receive();


            // 🔥 CRITICAL: commit using POST-TRANSITION manifold
            let qtm = Qtm::commit(
                &result.state,
                result.transition.net_work,
            );
            
            outputs.push(WalletOutput {
                coin: *coin,
                coordinate: qtm.coordinate,
                commitment: qtm.commitment,  // ✅ economic binding
            });
        }

        Ok(outputs)
    }
}
