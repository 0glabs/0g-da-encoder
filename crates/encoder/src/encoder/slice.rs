use super::{error::VerifierError, light_slice::LightEncodedSlice};
use crate::{
    amt::slice::EncodedSliceAMT,
    constants::{G1Curve, Scalar, COSET_N, G1A, PE},
    merkle::{slice::EncodedSliceMerkle, Bytes32},
    utils::scalar_to_h256,
    ZgSignerParams,
};
use amt::Proof;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std::cfg_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[derive(Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct EncodedSlice {
    pub index: usize,
    amt: EncodedSliceAMT,
    merkle: EncodedSliceMerkle,
}

impl PartialEq for EncodedSlice {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.amt == other.amt
            && self.merkle == other.merkle
    }
}

impl EncodedSlice {
    pub(super) fn new(
        index: usize, amt: EncodedSliceAMT, merkle: EncodedSliceMerkle,
    ) -> Self {
        Self { index, amt, merkle }
    }

    pub fn amt(&self) -> &EncodedSliceAMT { &self.amt }

    pub fn merkle(&self) -> &EncodedSliceMerkle { &self.merkle }

    pub(crate) fn check_merkle_idx(&self) -> Result<(), VerifierError> {
        if self.index != self.merkle.index() {
            Err(VerifierError::UnmatchedMerkleIndex {
                row_index: self.index,
                merkle_index: self.merkle.index(),
            })
        } else {
            Ok(())
        }
    }

    pub(crate) fn merkle_fields(
        &self,
    ) -> ([Bytes32; COSET_N], Vec<Bytes32>, Bytes32) {
        self.merkle.fields()
    }

    pub(crate) fn amt_fields(&self) -> (G1A, Proof<PE>, G1A) {
        self.amt.fields()
    }

    pub(crate) fn check_amt_idx(&self) -> Result<(), VerifierError> {
        if self.index != self.amt.index() {
            Err(VerifierError::UnmatchedAMTIndex {
                row_index: self.index,
                amt_index: self.amt.index(),
            })
        } else {
            Ok(())
        }
    }

    pub fn verify(
        &self, encoder_amt: &ZgSignerParams,
        authoritative_commitment: &G1Curve, authoritative_root: &[u8; 32],
    ) -> Result<(), VerifierError> {
        // consistency between amt and merkle
        // index consistency
        self.check_amt_idx()?;
        self.check_merkle_idx()?;
        // derive row_merkle from row_amt
        let row_amt = self.amt.row();
        let row_merkle: Vec<_> =
            row_amt.iter().map(|x| scalar_to_h256(*x)).collect();
        // verify amt, merkle
        self.amt.verify(encoder_amt, authoritative_commitment)?;
        self.merkle.verify(authoritative_root, row_merkle)?;
        Ok(())
    }
}

impl EncodedSlice {
    pub fn amt_row(&self) -> Vec<Scalar> { self.amt.row().clone() }

    pub fn merkle_row(&self) -> Vec<[u8; 32]> {
        cfg_iter!(self.amt.row())
            .cloned()
            .map(scalar_to_h256)
            .collect()
    }

    pub fn into_light_slice(&self) -> LightEncodedSlice {
        LightEncodedSlice::from_slice(self)
    }
}
