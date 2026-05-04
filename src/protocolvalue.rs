use quantom_value::{QuantPerm, DimensionObservation, Dimension};
use crate::protocol_id::QuantumId;
use crate::qp44::Heritage;
use blake3;

#[derive(Debug, Clone)]
pub struct Value {
    pub dimension: Dimension,
    pub structural_value: u128,
    pub activations: u64,
    pub density: Option<u128>,
}

#[repr(C)]
#[derive(Debug)]
pub struct Qtm {
    pub commitment: [u8; 32],
    pub coordinate: [u8; 32],
    pub net_work: u128,
    pub sigma: u128,
}


impl Qtm {
    pub fn economy(
        heritage: &Heritage,
    ) -> Value {
        
        let protocol = QuantumId::new();

        // 🔹 Build observation directly from Heritage
        let obs = DimensionObservation {
            dimension: heritage.state.dimension(),
            structural_value: heritage.state.structural_value(),
            activations: heritage.state.activations(),
            seed: heritage.transition.origin,
               };

        Self::from_observation(&obs, &protocol)
    }

    /// Build a ValueSlice from an observation
    pub fn from_observation(
        obs: &DimensionObservation,
        protocol: &QuantumId,
    ) -> Value {
        let density = protocol.density(obs);

        Value {
            dimension: obs.dimension,
            structural_value: obs.structural_value,
            activations: obs.activations,
            density,
        }
    }


    // ─────────────────────────────────────────────
    // 🔹 Coordinate (delegated from QP44)
    // ─────────────────────────────────────────────
    pub fn derive_coordinate(manifold: &QuantPerm) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(b"QP44::COORD::V1");
        h.update(&(manifold.dimension() as u64).to_le_bytes());
        h.update(&manifold.structural_value().to_le_bytes());
    
        let hash = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(hash.as_bytes());
        out
    }
    
    
    

        pub fn commit(
            manifold: &QuantPerm,
            net_work: u128,
        ) -> Self {
            use blake3;
    
            // 🔹 Deterministic coordinate from manifold geometry
            let coordinate =
                crate::protocolvalue::Qtm::derive_coordinate(manifold);
    
            // 🔹 Σ after transition (structural state)
            let sigma = manifold.structural_value();
    
            // 🔹 Pure commitment binding
            let mut h = blake3::Hasher::new();
    
            h.update(b"QTM::V1");
    
            // 🔐 Identity layer (geometry only)
            h.update(&coordinate);
    
            // 🔐 Economic state layer
            h.update(&sigma.to_le_bytes());
            h.update(&net_work.to_le_bytes());
    
            let hash = h.finalize();
    
            let mut commitment = [0u8; 32];
            commitment.copy_from_slice(hash.as_bytes());
    
            Qtm {
                commitment,
                coordinate,
                net_work,
                sigma,
            }
        }
    }