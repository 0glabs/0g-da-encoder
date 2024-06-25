use crate::{
    constants::{
        Scalar, BLOB_COL_LOG, BLOB_ROW_ENCODED, BLOB_ROW_LOG, BLOB_ROW_N,
        COSET_N, PE, RAW_BLOB_SIZE,
    },
    raw_blob::RawBlob,
    ZgEncoderParams,
};
use amt::{ec_algebra::G1Aff, HalfBlob};
use static_assertions::const_assert_eq;

use super::slice::EncodedSliceAMT;

pub struct EncodedBlobAMT([HalfBlob<PE, BLOB_COL_LOG, BLOB_ROW_LOG>; COSET_N]);
const_assert_eq!(ZgEncoderParams::len(), RAW_BLOB_SIZE);

impl EncodedBlobAMT {
    #[tracing::instrument(skip_all, name = "encode_amt", level = 2)]
    pub fn build(raw_blob: &RawBlob, encoder_amt: &ZgEncoderParams) -> Self {
        assert_eq!(raw_blob.len(), RAW_BLOB_SIZE);

        let answer = Self(encoder_amt.process_blob(raw_blob));

        answer.assert_commitment_consistent();

        answer
    }

    #[cfg(feature = "parallel")]
    pub(crate) fn iter_blob(
        &self,
    ) -> impl rayon::prelude::ParallelIterator<Item = &Scalar> {
        use rayon::prelude::*;
        self.0.par_iter().flat_map(|item| item.blob.par_iter())
    }

    #[cfg(not(feature = "parallel"))]
    pub(crate) fn iter_blob(&self) -> impl std::iter::Iterator<Item = &Scalar> {
        self.0.iter().flat_map(|item| item.blob.iter())
    }

    fn assert_commitment_consistent(&self) {
        let primary = &self.0[0];
        for blob in self.0.iter().skip(1) {
            assert_eq!(primary.commitment, blob.commitment);
        }
    }

    pub(crate) fn get_signer_row(&self, index: usize) -> EncodedSliceAMT {
        assert!(index < BLOB_ROW_ENCODED);
        let coset = index / BLOB_ROW_N;
        EncodedSliceAMT::new(
            index,
            self.0[coset].commitment,
            self.0[coset].get_row(index % BLOB_ROW_N),
        )
    }

    pub(crate) fn get_commitment(&self) -> G1Aff<PE> {
        self.0[0].commitment
    }

    #[cfg(any(test, feature = "testonly_code"))]
    pub(crate) fn get_invalid_row(
        &self, index: usize, err_code: &ErrCodeAMT,
    ) -> EncodedSliceAMT {
        use crate::constants::G1A;
        use ark_ec::{AffineRepr, CurveGroup};
        use ark_ff::One;

        assert!(index < BLOB_ROW_ENCODED);

        self.assert_commitment_consistent();
        let mut commitment = self.get_commitment();
        let mut row = self.0[index / BLOB_ROW_N].get_row(index % BLOB_ROW_N);
        match err_code {
            ErrCodeAMT::WrongIndex => row.index += 1,
            ErrCodeAMT::WrongRow => row.row[0] += Scalar::one(),
            ErrCodeAMT::WrongCommitment => {
                commitment = (commitment + G1A::generator()).into_affine();
            } /* TODO WrongProof has not been tested */
            ErrCodeAMT::IncorrectHighCommitment => {
                row.high_commitment =
                    (row.high_commitment + G1A::generator()).into_affine()
            }
        }
        EncodedSliceAMT::new(index, commitment, row)
    }
}

#[cfg(any(test, feature = "testonly_code"))]
#[derive(Debug, Eq, Hash, PartialEq)]
pub enum ErrCodeAMT {
    WrongIndex,
    WrongRow,
    WrongCommitment,
    IncorrectHighCommitment,
}
