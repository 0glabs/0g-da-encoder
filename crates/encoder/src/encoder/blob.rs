use super::slice::EncodedSlice;
use crate::{
    amt::blob::EncodedBlobAMT,
    constants::{G1Curve, BLOB_ROW_ENCODED, COSET_N, G1A, RAW_BLOB_SIZE},
    merkle::{blob::EncodedBlobMerkle, Bytes32},
    raw_blob::RawBlob,
    utils::{keccak_tuple, scalar_to_h256},
    ZgEncoderParams,
};

#[cfg(any(test, feature = "testonly_code"))]
use crate::ZgSignerParams;

#[cfg(any(test, feature = "testonly_code"))]
use crate::{
    amt::blob::ErrCodeAMT, amt::error::AmtError, encoder::error::VerifierError,
    merkle::blob::ErrCodeMerkle, merkle::error::MerkleError,
};
#[cfg(any(test, feature = "testonly_code"))]
use std::collections::HashMap;

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use static_assertions::const_assert;

pub struct EncodedBlob {
    amt: EncodedBlobAMT,
    merkle: EncodedBlobMerkle,
}

impl EncodedBlob {
    pub fn build(raw_blob: &RawBlob, encoder_amt: &ZgEncoderParams) -> Self {
        assert_eq!(raw_blob.len(), RAW_BLOB_SIZE);

        let amt = EncodedBlobAMT::build(raw_blob, encoder_amt);

        let blob_h256: Vec<_> =
            amt.iter_blob().cloned().map(scalar_to_h256).collect();
        let merkle = EncodedBlobMerkle::build(blob_h256);

        Self { amt, merkle }
    }

    pub fn get_row(&self, index: usize) -> EncodedSlice {
        assert!(index < BLOB_ROW_ENCODED);
        let amt = self.amt.get_signer_row(index);
        let merkle = self.merkle.get_row(index);
        EncodedSlice::new(index, amt, merkle)
    }

    pub fn get_affine_commitment(&self) -> G1A { self.amt.get_commitment() }

    pub fn get_commitment(&self) -> G1Curve { self.amt.get_commitment().into() }

    pub fn get_roots(&self) -> [Bytes32; COSET_N] { self.merkle.root() }

    pub fn get_file_root(&self) -> Bytes32 {
        compute_file_root(&self.get_roots())
    }

    pub fn get_data(&self) -> &Vec<Bytes32> { &self.merkle.data }
}

pub fn compute_file_root(roots: &[Bytes32; COSET_N]) -> Bytes32 {
    const_assert!(COSET_N <= 3);
    match COSET_N {
        1 => roots[0],
        2 => keccak_tuple(roots[0], roots[1]),
        3 => keccak_tuple(keccak_tuple(roots[0], roots[1]), roots[2]),
        _ => unimplemented!(),
    }
}

#[cfg(any(test, feature = "testonly_code"))]
#[derive(Debug, Eq, Hash, PartialEq)]
pub enum ErrCode {
    AMT(ErrCodeAMT),
    Merkle(ErrCodeMerkle),
    WrongIndex,
    WrongAmtIndex,
    WrongMerkleIndex,
}

#[cfg(any(test, feature = "testonly_code"))]
pub fn gen_err_signer_map(index: usize) -> HashMap<ErrCode, VerifierError> {
    use crate::constants::BLOB_ROW_N;

    let mut err_signer_map = HashMap::new();
    // index
    err_signer_map.insert(
        ErrCode::WrongIndex,
        VerifierError::UnmatchedAMTIndex {
            row_index: index + 1,
            amt_index: index,
        },
    );
    if index < BLOB_ROW_ENCODED - 1 {
        err_signer_map.insert(
            ErrCode::WrongAmtIndex,
            VerifierError::UnmatchedAMTIndex {
                row_index: index,
                amt_index: index + 1,
            },
        );
        err_signer_map.insert(
            ErrCode::WrongMerkleIndex,
            VerifierError::UnmatchedMerkleIndex {
                row_index: index,
                merkle_index: index + 1,
            },
        );
    } // else {ErrorEncoder in gen_err_encoder_map() will be triggered instead of
      // ErrorSigner} amt
    err_signer_map.insert(
        ErrCode::AMT(ErrCodeAMT::WrongRow),
        VerifierError::AMT(AmtError::IncorrectProof {
            coset_index: index / BLOB_ROW_N,
            amt_index: index % BLOB_ROW_N,
            error: amt::AmtProofError::InconsistentCommitment,
        }),
    );
    err_signer_map.insert(
        ErrCode::AMT(ErrCodeAMT::WrongIndex),
        VerifierError::AMT(AmtError::UnmatchedCosetIndex {
            coset_index: index / BLOB_ROW_N,
            local_index: index % BLOB_ROW_N,
            amt_index: (index % BLOB_ROW_N) + 1,
        }),
    );
    err_signer_map.insert(
        ErrCode::AMT(ErrCodeAMT::WrongCommitment),
        VerifierError::AMT(AmtError::IncorrectCommitment),
    );
    err_signer_map.insert(
        ErrCode::AMT(ErrCodeAMT::IncorrectHighCommitment),
        VerifierError::AMT(AmtError::IncorrectProof {
            coset_index: index / BLOB_ROW_N,
            amt_index: index % BLOB_ROW_N,
            error: amt::AmtProofError::FailedLowDegreeTest,
        }),
    );
    // merkle
    err_signer_map.insert(
        ErrCode::Merkle(ErrCodeMerkle::WrongIndex),
        VerifierError::UnmatchedMerkleIndex {
            row_index: index,
            merkle_index: index + 1,
        },
    );
    err_signer_map.insert(
        ErrCode::Merkle(ErrCodeMerkle::WrongLocalRoot),
        VerifierError::Merkle(MerkleError::IncorrectLocalRoot {
            row_index: index,
        }),
    );
    err_signer_map.insert(
        ErrCode::Merkle(ErrCodeMerkle::WrongProof),
        VerifierError::Merkle(MerkleError::IncorrectProof { row_index: index }),
    );
    err_signer_map.insert(
        ErrCode::Merkle(ErrCodeMerkle::WrongRoot),
        VerifierError::Merkle(MerkleError::IncorrectRoot),
    );
    err_signer_map
}

impl EncodedBlob {
    #[cfg(any(test, feature = "testonly_code"))]
    fn get_invalid_row(
        &self, index: usize, err_code: &ErrCode,
    ) -> EncodedSlice {
        assert!(index < BLOB_ROW_ENCODED);
        let mut global_index = index;
        let mut amt_index = index;
        let mut merkle_index = index;
        match err_code {
            ErrCode::WrongIndex => global_index += 1,
            ErrCode::WrongAmtIndex => amt_index += 1,
            ErrCode::WrongMerkleIndex => merkle_index += 1,
            _ => (),
        };
        let amt = match err_code {
            ErrCode::AMT(err_code_amt) => {
                self.amt.get_invalid_row(amt_index, err_code_amt)
            }
            _ => self.amt.get_signer_row(amt_index),
        };
        let merkle = match err_code {
            ErrCode::Merkle(err_code_merkle) => {
                self.merkle.get_invalid_row(merkle_index, err_code_merkle)
            }
            _ => self.merkle.get_row(merkle_index),
        };
        EncodedSlice::new(global_index, amt, merkle)
    }

    #[cfg(any(test, feature = "testonly_code"))]
    pub fn test_verify(&self, encoder_amt: &ZgSignerParams) {
        let authoritative_commitment = self.get_commitment();
        let authoritative_root = self.get_file_root();

        // verify
        for index in 0..BLOB_ROW_ENCODED {
            let encoded_slice = self.get_row(index);
            encoded_slice
                .verify(
                    &encoder_amt,
                    &authoritative_commitment,
                    &authoritative_root,
                )
                .unwrap();
        }

        for index in 0..BLOB_ROW_ENCODED {
            let err_signer_map = gen_err_signer_map(index);
            for (err_code, expected_err_signer) in err_signer_map.iter() {
                let invalid_indexed_slice =
                    self.get_invalid_row(index, err_code);
                let err_signer = invalid_indexed_slice.verify(
                    &encoder_amt,
                    &authoritative_commitment,
                    &authoritative_root,
                );
                assert_eq!(err_signer.as_ref(), Err(expected_err_signer));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EncodedBlob;
    use crate::{
        constants::{BLOB_ROW_ENCODED, MAX_BLOB_SIZE},
        encoder::error::EncoderError,
        raw_blob::RawBlob,
        raw_data::RawData,
        ZgEncoderParams, ZgSignerParams,
    };
    use amt::{EncoderParams, VerifierParams};
    use once_cell::sync::Lazy;
    use rand::{rngs::StdRng, Rng, SeedableRng};
    use test_case::test_case;

    static ENCODER: Lazy<ZgEncoderParams> =
        Lazy::new(|| EncoderParams::from_dir_mont("../amt/pp", true, None));
    static SIGNER: Lazy<ZgSignerParams> = Lazy::new(|| {
        let _ = &*ENCODER;
        VerifierParams::from_dir_mont("../amt/pp")
    });

    fn gen_encoded_blob(num_bytes: usize) -> Result<EncodedBlob, EncoderError> {
        // generate input
        let seed = 222u64;
        let mut rng = StdRng::seed_from_u64(seed);
        let mut data = vec![0u8; num_bytes];
        rng.fill(&mut data[..]);
        // batcher
        let raw_data: RawData = data[..].try_into()?;
        let raw_blob: RawBlob = raw_data.try_into().unwrap();
        let encoded_blob = EncodedBlob::build(&raw_blob, &ENCODER);
        Ok(encoded_blob)
    }

    #[test]
    fn test_light_slice() -> () {
        let num_bytes = 1234;
        let encoded_blob = gen_encoded_blob(num_bytes).unwrap();

        for index in 0..BLOB_ROW_ENCODED {
            let encoded_slice = encoded_blob.get_row(index);
            let row = encoded_slice.amt_row();
            let light_slice = encoded_slice.into_light_slice();
            let encoded_slice_recovered = light_slice.into_slice(row);
            assert_eq!(encoded_slice, encoded_slice_recovered);
        }
    }

    #[test_case(0 => Ok(()); "zero sized data")]
    #[test_case(1 => Ok(()); "one sized data")]
    #[test_case(1234 => Ok(()); "normal sized data")]
    #[test_case(MAX_BLOB_SIZE => Ok(()); "exact sized data")]
    #[test_case(MAX_BLOB_SIZE + 1 => Err(EncoderError::TooLargeBlob { actual: MAX_BLOB_SIZE + 1, expected_max: MAX_BLOB_SIZE }); "overflow sized data")]
    fn test_batcher_and_verify(num_bytes: usize) -> Result<(), EncoderError> {
        let encoded_blob = gen_encoded_blob(num_bytes)?;

        encoded_blob.test_verify(&SIGNER);

        Ok(())
    }
}
