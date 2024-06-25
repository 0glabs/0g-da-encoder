use super::{error::MerkleError, Bytes32};
use crate::{
    constants::{
        BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N, COSET_N, RAW_BLOB_SIZE,
    },
    encoder::blob::compute_file_root,
    utils::keccak_chunked,
};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use keccak_hash::keccak;

#[derive(Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct EncodedSliceMerkle {
    // index: 0, 1, ...,
    // BLOB_ROW_ENCODED
    root: [Bytes32; COSET_N],
    proof: Vec<Bytes32>,
    leaf_index: usize,
    leaf: Bytes32,
}

impl PartialEq for EncodedSliceMerkle {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.proof == other.proof
            && self.leaf_index == other.leaf_index
            && self.leaf == other.leaf
    }
}

impl EncodedSliceMerkle {
    pub(crate) fn new(
        root: [Bytes32; COSET_N], proof: Vec<Bytes32>, leaf_index: usize,
        leaf: Bytes32,
    ) -> Self {
        Self {
            root,
            proof,
            leaf_index,
            leaf,
        }
    }

    pub(crate) fn index(&self) -> usize {
        self.leaf_index
    }

    pub(crate) fn fields(&self) -> ([Bytes32; COSET_N], Vec<Bytes32>, Bytes32) {
        (self.root, self.proof.clone(), self.leaf)
    }

    pub(crate) fn verify(
        &self, authoritative_root: &Bytes32, row: Vec<Bytes32>,
    ) -> Result<(), MerkleError> {
        // verify authoritative_root
        if compute_file_root(&self.root) != *authoritative_root {
            return Err(MerkleError::IncorrectRoot);
        }
        // verify row.len() (local)
        if row.len() != BLOB_COL_N {
            return Err(MerkleError::IncorrectSize {
                actual: row.len(),
                expected: BLOB_COL_N,
            });
        }
        // verify leaf_index (global)
        if self.leaf_index >= BLOB_ROW_ENCODED {
            return Err(MerkleError::RowIndexOverflow {
                actual: self.leaf_index,
                expected_max: BLOB_ROW_ENCODED,
            });
        }

        // verify Merkle local
        let leaves = keccak_chunked(&row, 8);

        let mut last_layer = leaves;
        while last_layer.len() > 1 {
            let next_layer = keccak_chunked(&last_layer, 2);
            let mut to_push_layer = next_layer;
            std::mem::swap(&mut last_layer, &mut to_push_layer);
        }

        let row_merkle_root = last_layer[0];
        if row_merkle_root != self.leaf {
            return Err(MerkleError::IncorrectLocalRoot {
                row_index: self.leaf_index,
            });
        }
        // verify Merkle global
        let mut position: usize = self.leaf_index % RAW_BLOB_SIZE;
        let computed =
            self.proof.clone().into_iter().fold(self.leaf, |a, b| {
                let x = if position % 2 == 1 { [b, a] } else { [a, b] };
                let x: [u8; 64] = unsafe { std::mem::transmute(x) };
                position >>= 1;
                keccak(x).0
            });
        let verify_global = computed == self.root[self.leaf_index / BLOB_ROW_N];
        if !verify_global {
            return Err(MerkleError::IncorrectProof {
                row_index: self.leaf_index,
            });
        }
        Ok(())
    }
}
