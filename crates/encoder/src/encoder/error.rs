pub use crate::{amt::error::AmtError, merkle::error::MerkleError};

#[derive(Debug, PartialEq, Eq)]
pub enum EncoderError {
    TooLargeBlob { actual: usize, expected_max: usize },
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifierError {
    #[allow(clippy::upper_case_acronyms)]
    AMT(AmtError),
    Merkle(MerkleError),
    UnmatchedAMTIndex {
        row_index: usize,
        amt_index: usize,
    },
    UnmatchedMerkleIndex {
        row_index: usize,
        merkle_index: usize,
    },
}

impl From<EncoderError> for String {
    fn from(error: EncoderError) -> String {
        format!("{:?}", error)
    }
}

impl From<AmtError> for VerifierError {
    fn from(error: AmtError) -> Self {
        VerifierError::AMT(error)
    }
}

impl From<MerkleError> for VerifierError {
    fn from(error: MerkleError) -> Self {
        VerifierError::Merkle(error)
    }
}
