use std::time::{Duration, Instant};

use amt::DeferredVerifier;
use ark_bn254::Bn254;
use rand::{rngs::OsRng, Rng};
use zg_encoder::{
    constants::{G1Curve, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N, RAW_UNIT},
    EncodedBlob, EncodedSlice, RawBlob, RawData, ZgEncoderParams,
    ZgSignerParams,
};

fn main() {
    let param_dir = "../crates/amt/pp";
    let params = ZgEncoderParams::from_dir_mont(param_dir, true, None);
    let ver_params = ZgSignerParams::from_dir_mont(param_dir);

    let mut total_duration = Duration::from_secs(0);

    for _ in 0..10 {
        let (commitment, root, slices) = make_slices(&params);
        let start = Instant::now();
        for slice in slices {
            slice.verify(&ver_params, &commitment, &root, None).unwrap();
        }
        total_duration += start.elapsed();
    }
    println!(
        "Original: {:?}  / line",
        total_duration / BLOB_ROW_ENCODED as u32 / 10
    );

    let defered_verifier = DeferredVerifier::<Bn254>::new();
    let mut instant_duration = Duration::from_secs(0);
    for _ in 0..10 {
        let (commitment, root, slices) = make_slices(&params);
        let start = Instant::now();
        for slice in slices {
            slice
                .verify(
                    &ver_params,
                    &commitment,
                    &root,
                    Some(defered_verifier.clone()),
                )
                .unwrap();
        }
        instant_duration += start.elapsed();
    }
    instant_duration /= BLOB_ROW_ENCODED as u32 * 10;
    let start = Instant::now();
    assert!(defered_verifier.fast_check());
    let deferred_duration = start.elapsed() / BLOB_ROW_ENCODED as u32 / 10;
    println!(
        "Optimized Total elapsed {:?} + {:?} = {:?} / line",
        instant_duration,
        deferred_duration,
        instant_duration + deferred_duration
    );
}

fn make_slices(
    params: &ZgEncoderParams,
) -> (G1Curve, [u8; 32], Vec<EncodedSlice>) {
    let mut data = vec![0u8; RAW_UNIT * BLOB_ROW_N * BLOB_COL_N];
    OsRng.fill(&mut data[..]);
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.into();

    let encoded_blob = EncodedBlob::build(&raw_blob, params);

    (
        encoded_blob.get_commitment(),
        encoded_blob.get_file_root(),
        (0..BLOB_ROW_ENCODED)
            .map(|i| encoded_blob.get_row(i))
            .collect(),
    )
}
