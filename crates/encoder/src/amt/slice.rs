use super::error::AmtError;
use crate::{
    constants::{
        G1Curve, Scalar, BLOB_COL_LOG, BLOB_COL_N, BLOB_ROW_ENCODED,
        BLOB_ROW_LOG, BLOB_ROW_N, PE,
    },
    ZgSignerParams,
};
use amt::{BlobRow, Proof};
use ark_ec::CurveGroup;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

#[derive(Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct EncodedSliceAMT {
    pub index: usize, /* index: 0, 1, ..., BLOB_ROW_ENCODED - 1 */
    pub commitment: G1Curve,
    pub row: BlobRow<PE, BLOB_COL_LOG, BLOB_ROW_LOG>, /* index in half, row,
                                                       * proof */
}

impl PartialEq for EncodedSliceAMT {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.commitment == other.commitment
            && self.row == other.row
    }
}

impl EncodedSliceAMT {
    pub(crate) fn new(
        index: usize, commitment: G1Curve,
        row: BlobRow<PE, BLOB_COL_LOG, BLOB_ROW_LOG>,
    ) -> Self {
        Self {
            index,
            commitment,
            row,
        }
    }

    pub(crate) fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn fields(&self) -> (G1Curve, Proof<PE>, G1Curve) {
        (
            self.commitment,
            self.row.proof.clone(),
            self.row.high_commitment,
        )
    }

    pub(crate) fn row(&self) -> Vec<Scalar> {
        self.row.row.clone()
    }

    pub(crate) fn verify(
        &self, encoder_amt: &ZgSignerParams, authoritative_commitment: &G1Curve,
    ) -> Result<(), AmtError> {
        // verify authoritative_commitment
        if self.commitment.into_affine()
            != authoritative_commitment.into_affine()
        {
            return Err(AmtError::IncorrectCommitment);
        }
        // verify row.len() (local)
        if self.row.row.len() != BLOB_COL_N {
            return Err(AmtError::IncorrectRowSize {
                actual: self.row.row.len(),
                expected: BLOB_COL_N,
            });
        }
        // verify index (global)
        if self.index >= BLOB_ROW_ENCODED {
            return Err(AmtError::RowIndexOverflow {
                actual: self.index,
                expected_max: BLOB_ROW_ENCODED,
            });
        }
        // verify index & proof for
        // primary and coset
        // case-by-case

        let coset_idx = self.index / BLOB_ROW_N;
        let local_idx = self.index % BLOB_ROW_N;
        if local_idx != self.row.index {
            return Err(AmtError::UnmatchedCosetIndex {
                coset_index: coset_idx,
                local_index: local_idx,
                amt_index: self.row.index,
            });
        }

        self.row
            .verify(&encoder_amt.amt_list[coset_idx], self.commitment)
            .map_err(|err| AmtError::IncorrectProof {
                coset_index: coset_idx,
                amt_index: self.row.index,
                error: err,
            })?;

        Ok(())
    }
}
