#![allow(unused)]

use std::time::Instant;

use ark_std::cfg_into_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use tonic::{Code, Request, Response, Status};
use tracing::{debug, info, instrument};

pub mod encoder {
    tonic::include_proto!("encoder");
}

pub use encoder::encoder_server::EncoderServer;
use encoder::{encoder_server::Encoder, EncodeBlobReply, EncodeBlobRequest};

use amt::{
    ec_algebra::{CanonicalSerialize, CurveGroup},
    EncoderParams, PowerTau, VerifierParams,
};
use zg_encoder::{
    constants::{
        Scalar, BLOB_COL_LOG, BLOB_ROW_ENCODED, BLOB_ROW_LOG, COSET_N, PE,
    },
    EncodedBlob, EncodedSlice, EncoderError, RawBlob, RawData, ZgEncoderParams,
    ZgSignerParams,
};

pub struct EncoderService {
    pub params: ZgEncoderParams, /* TODO: remove pub */
}

impl EncoderService {
    pub fn new(param_dir: &str) -> Self {
        let params = EncoderParams::from_dir_mont(param_dir, false, None);
        Self { params }
    }

    pub fn new_for_test(param_dir: &str) -> Self {
        let params = EncoderParams::from_dir_mont(param_dir, true, None);
        Self { params }
    }
}

#[tonic::async_trait]
impl Encoder for EncoderService {
    async fn encode_blob(
        &self, request: Request<EncodeBlobRequest>,
    ) -> Result<Response<EncodeBlobReply>, Status> {
        let remote_addr = request.remote_addr();
        let request_content = request.into_inner();
        info!(
            requester = ?remote_addr,
            data_lenth = request_content.data.len(),
            "Receive encoder task",
        );

        let reply = self
            .process_data(&request_content.data, request_content.require_data)
            .map_err(|e| Status::new(Code::Unknown, e))?;

        Ok(Response::new(reply))
    }
}

impl EncoderService {
    #[instrument(skip_all, name = "encode", level = 2)]
    pub fn process_data(
        &self, data: &[u8], require_data: bool,
    ) -> Result<EncodeBlobReply, EncoderError> {
        let raw_data: RawData = data.try_into()?;
        let raw_blob: RawBlob = raw_data.into();

        let encoded_blob = EncodedBlob::build(&raw_blob, &self.params);

        let erasure_commitment = {
            let c = encoded_blob.get_affine_commitment();
            let mut answer: Vec<u8> = Vec::new();
            c.x.serialize_uncompressed(&mut answer).unwrap();
            c.y.serialize_uncompressed(&mut answer).unwrap();
            answer
        };
        let storage_root = encoded_blob.get_file_root().to_vec();
        let encoded_data = if require_data {
            let data = encoded_blob.get_data();
            let ptr = &data[0][0] as *const u8;
            unsafe { std::slice::from_raw_parts(ptr, data.len() * 32).to_vec() }
        } else {
            vec![]
        };

        let encoded_slice: Vec<Vec<u8>> = cfg_into_iter!(0..BLOB_ROW_ENCODED)
            .map(|row_idx| serailize_to_bytes(&encoded_blob.get_row(row_idx)))
            .collect();

        let reply = EncodeBlobReply {
            version: 0,
            erasure_commitment,
            storage_root,
            encoded_data,
            encoded_slice,
        };
        Ok(reply)
    }
}

pub struct SignerService {
    pub params: ZgSignerParams,
}

impl SignerService {
    pub fn new(param_dir: &str) -> Self {
        let params = VerifierParams::from_dir_mont(param_dir);
        Self { params }
    }
}

#[cfg(test)]
impl SignerService {
    pub fn deserialize_reply(
        &self, reply: EncodeBlobReply, encoded_data: &EncodedBlob,
    ) {
        use amt::ec_algebra::CanonicalDeserialize;
        use ark_bn254::{Fq, G1Affine, G1Projective};
        use zg_encoder::constants::G1Curve;
        // deserialize

        let erasure_commitment: G1Projective = {
            let mut raw_commitment = &*reply.erasure_commitment;
            let x = Fq::deserialize_uncompressed(&mut raw_commitment).unwrap();
            let y = Fq::deserialize_uncompressed(&mut raw_commitment).unwrap();
            G1Affine::new(x, y).into()
        };

        let storage_root =
            <[u8; 32]>::deserialize_uncompressed(&*reply.storage_root).unwrap();
        let encoded_data_h256: Vec<_> = reply
            .encoded_data
            .chunks_exact(32)
            .map(|x| <[u8; 32]>::deserialize_uncompressed(&*x).unwrap())
            .collect();
        let encoded_slice: Vec<_> = reply
            .encoded_slice
            .iter()
            .map(|row| {
                EncodedSlice::deserialize_uncompressed(&*row.to_vec()).unwrap()
            })
            .collect();
        // test consistency
        assert_eq!(erasure_commitment, encoded_data.get_commitment());
        assert_eq!(storage_root, encoded_data.get_file_root());
        assert_eq!(encoded_data.get_data().len(), encoded_data_h256.len());
        for index in 0..BLOB_ROW_ENCODED {
            assert_eq!(encoded_slice[index], encoded_data.get_row(index));
        }
        // test verify
        encoded_data.test_verify(&self.params);
    }
}

fn serailize_to_bytes<T: CanonicalSerialize>(data: &T) -> Vec<u8> {
    let mut answer: Vec<u8> = Vec::new();
    data.serialize_uncompressed(&mut answer).unwrap();
    answer
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, Rng, SeedableRng};
    use test_case::test_case;
    use zg_encoder::{
        constants::{MAX_BLOB_SIZE, MAX_RAW_DATA_SIZE},
        EncodedBlob, EncoderError, RawBlob, RawData,
    };

    use crate::{EncoderService, SignerService};

    use once_cell::sync::Lazy;
    const PARAM_DIR: &str = "../crates/amt/pp";
    static ENCODER_SERVICE: Lazy<EncoderService> =
        Lazy::new(|| EncoderService::new_for_test(PARAM_DIR));
    static SIGNER_SERVICE: Lazy<SignerService> = Lazy::new(|| {
        Lazy::force(&ENCODER_SERVICE);
        SignerService::new(PARAM_DIR)
    });

    #[test_case(1 => Ok(()); "one sized data")]
    #[test_case(1234 => Ok(()); "normal sized data")]
    #[test_case(MAX_RAW_DATA_SIZE => Ok(()); "exact sized data")]
    #[test_case(MAX_RAW_DATA_SIZE + 1 => Err(EncoderError::TooLargeBlob{actual: MAX_RAW_DATA_SIZE + 1, expected_max: MAX_RAW_DATA_SIZE}); "overflow sized data")]
    fn test_serde(num_bytes: usize) -> Result<(), EncoderError> {
        let seed = 22u64;
        let mut rng = StdRng::seed_from_u64(seed);

        for _ in 0..3 {
            // generate input
            let mut data = vec![0u8; num_bytes];
            rng.fill(&mut data[..]);
            // serialize
            let reply = ENCODER_SERVICE.process_data(&data, true)?;
            // ground truth
            let raw_data: RawData = data[..].try_into().unwrap();
            let raw_blob: RawBlob = raw_data.into();
            let encoded_data =
                EncodedBlob::build(&raw_blob, &ENCODER_SERVICE.params);
            // deserialize
            SIGNER_SERVICE.deserialize_reply(reply, &encoded_data);
        }
        Ok(())
    }
}
