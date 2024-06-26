use ark_bn254::{Bn254, Fr, G1Affine, G1Projective, G2Projective};
use ark_ff::FftField;
use static_assertions::const_assert;

pub type PE = Bn254;
pub type Scalar = Fr;
pub type G1A = G1Affine;
pub type G1Curve = G1Projective;
pub type G2Curve = G2Projective;

const TEST_SETTING: bool = (cfg!(test) || cfg!(feature = "testonly_code"))
    && !cfg!(feature = "production_mode");

pub const BLOB_ROW_LOG: usize = if TEST_SETTING { 6 } else { 10 };
pub const BLOB_COL_LOG: usize = if TEST_SETTING { 5 } else { 10 };

pub const COSET_N: usize = 3;

pub const RAW_UNIT: usize = 31;
pub const BLOB_UNIT: usize = 32;

pub const BLOB_ROW_N: usize = 1 << BLOB_ROW_LOG;
pub const BLOB_ROW_ENCODED: usize = BLOB_ROW_N * COSET_N;
pub const BLOB_COL_N: usize = 1 << BLOB_COL_LOG;
pub const RAW_BLOB_SIZE: usize = BLOB_ROW_N * BLOB_COL_N;
pub const ENCODED_BLOB_SIZE: usize = BLOB_ROW_ENCODED * BLOB_COL_N;
pub const MAX_BLOB_SIZE: usize = RAW_UNIT * BLOB_ROW_N * BLOB_COL_N;

const_assert!(1usize << <Scalar as FftField>::TWO_ADICITY >= ENCODED_BLOB_SIZE);
