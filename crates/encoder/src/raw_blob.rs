use crate::{
    cfg_chunks_exact,
    constants::{Scalar, MAX_BLOB_SIZE, RAW_UNIT},
    raw_data::RawData,
    utils::raw_unit_to_scalar,
};
use std::ops::{Deref, DerefMut};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub struct RawBlob(Vec<Scalar>); // BLOB_ROW_N * BLOB_COL_N

impl Deref for RawBlob {
    type Target = [Scalar];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for RawBlob {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl From<RawData> for RawBlob {
    fn from(data: RawData) -> Self {
        // raw_data_to_raw_blob
        assert_eq!(data.len(), MAX_BLOB_SIZE);
        let raw_blob_1d: Vec<_> = cfg_chunks_exact!(data, RAW_UNIT)
            .map(raw_unit_to_scalar)
            .collect();
        RawBlob(raw_blob_1d)
    }
}
