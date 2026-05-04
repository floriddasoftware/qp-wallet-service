use quantom_value::DimensionObservation;

/// Quantum Identity — the deterministic anchor for all protocol behavior.
/// Provides quantum seed, density, and safety valves for economic slices.
#[derive(Debug, Clone, Copy)]
pub struct QuantumId;

impl QuantumId {
    pub const fn new() -> Self {
        Self
    }

    // --------------------------------------------------
    // 🔹 Quantum Execution Seed
    // --------------------------------------------------

    /// The quantum seed is the deterministic substrate for all transitions.
    pub fn quantum_seed(
        &self,
        proof: impl crate::economic_gate::BalanceProof,
    ) -> [u8; 32] {
        crate::config::require_seed(proof)
    }

    // --------------------------------------------------
    // 🔹 Structural Density
    // --------------------------------------------------

    /// Density = Σ / activations
    pub fn density(
        &self,
        obs: &DimensionObservation,
    ) -> Option<u128> {
        if obs.activations == 0 {
            None
        } else {
            Some(obs.structural_value / obs.activations as u128)
        }
    }

    // --------------------------------------------------
    // 🔹 Autonomous Locked Debt (Safety Valve)
    // --------------------------------------------------

    /// LockedDebt = min(Density, τ)
    ///
    /// τ must be supplied by the caller (observer layer).
    pub fn locked_debt(
        &self,
        obs: &DimensionObservation,
        tau: u128,
    ) -> Option<u128> {
        self.density(obs)
            .map(|density| core::cmp::min(density, tau))
    }

    // --------------------------------------------------
    // 🔹 Maximum Lendable
    // --------------------------------------------------

    /// The maximum lendable quantum value is simply Σ.
    pub fn max_lendable(
        &self,
        obs: &DimensionObservation,
    ) -> Option<u128> {
        Some(obs.structural_value)
    }
}

/// A structural slice that is economically locked under quantum identity.
#[derive(Debug, Clone, Copy)]
pub struct LockedStructure {
    pub dimension: u64,
    pub structural_value: u128,
}
