#[macro_use]
extern crate tracing;

mod blob;
pub mod ec_algebra;
mod error;
mod power_tau;
mod proofs;
mod prove_params;
mod utils;
mod verify_params;

pub use blob::{
    encode::{BlobRow, EncoderParams, HalfBlob},
    verify::VerifierParams,
};
pub use power_tau::PowerTau;
pub use proofs::{AmtProofError, Proof};
pub use prove_params::AMTParams;
pub use utils::{amtp_file_name, amtp_verify_file_name, ptau_file_name};
pub use verify_params::AMTVerifyParams;

#[cfg(not(feature = "cuda-bls12-381"))]
pub use prove_params::fast_serde_bn254;

pub use blob::encode::to_coset_blob;
pub use utils::{bitreverse, change_matrix_direction};
