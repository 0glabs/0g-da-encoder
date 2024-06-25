use crate::{constants::MAX_BLOB_SIZE, encoder::error::EncoderError};
use std::ops::{Deref, DerefMut};

#[derive(Clone)]
pub struct RawData(Vec<u8>); // MAX_BLOB_SIZE

impl Default for RawData {
    fn default() -> Self {
        RawData(vec![0u8; MAX_BLOB_SIZE])
    }
}

impl Deref for RawData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RawData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TryFrom<&[u8]> for RawData {
    type Error = EncoderError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let value_len = value.len();
        if value_len <= MAX_BLOB_SIZE {
            let mut array = vec![0u8; MAX_BLOB_SIZE];
            array[..value_len].copy_from_slice(value);
            Ok(RawData(array))
        } else {
            Err(EncoderError::TooLargeBlob {
                actual: value.len(),
                expected_max: MAX_BLOB_SIZE,
            })
        }
    }
}
