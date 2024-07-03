#[macro_use]
extern crate tracing;

#[macro_use]
extern crate ark_std;

use std::time::{Duration, Instant};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use amt::DeferredVerifier;
use ark_bn254::Bn254;
use rand::{rngs::OsRng, Rng};
use tracing::Level;

use zg_encoder::{
    constants::{G1Curve, BLOB_ROW_ENCODED, MAX_RAW_DATA_SIZE, RAW_BLOB_SIZE},
    EncodedBlob, EncodedSlice, RawBlob, RawData, ZgEncoderParams,
    ZgSignerParams,
};

const BATCHES: usize = 20000 / RAW_BLOB_SIZE + 1;

fn init_logger() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_target(false)
        .init();
}

fn main() {
    init_logger();

    #[cfg(feature = "cuda")]
    ag_cuda_ec::init_local_workspace();

    info!("load params");
    let param_dir = "../crates/amt/pp";
    let params = ZgEncoderParams::from_dir_mont(param_dir, true, None);
    let ver_params = ZgSignerParams::from_dir_mont(param_dir);

    bench_no_defer(&params, &ver_params);
    bench_defer(&params, &ver_params);
}

fn bench_no_defer(params: &ZgEncoderParams, ver_params: &ZgSignerParams) {
    let mut total_duration = Duration::from_secs(0);

    for round in 0..BATCHES {
        debug!(round, "make slice");
        let (commitment, root, slices) = make_slices(&params);

        debug!(round, "start verify");
        let start = Instant::now();
        cfg_iter!(slices).for_each(|slice| {
            slice.verify(&ver_params, &commitment, &root, None).unwrap()
        });
        total_duration += start.elapsed();
    }
    info!(
        "Non deferred verifier: {:?} per line",
        total_duration / BLOB_ROW_ENCODED as u32 / BATCHES as u32
    );
}

fn bench_defer(params: &ZgEncoderParams, ver_params: &ZgSignerParams) {
    let defered_verifier = DeferredVerifier::<Bn254>::new();
    let mut instant_duration = Duration::from_secs(0);
    for round in 0..BATCHES {
        debug!(round, "make slice");
        let (commitment, root, slices) = make_slices(&params);

        let start = Instant::now();
        debug!(round, "start verify");
        cfg_iter!(slices).for_each(|slice| {
            slice
                .verify(
                    &ver_params,
                    &commitment,
                    &root,
                    Some(defered_verifier.clone()),
                )
                .unwrap()
        });
        instant_duration += start.elapsed();
    }
    instant_duration /= (BLOB_ROW_ENCODED * BATCHES) as u32;

    let start = Instant::now();

    #[cfg(feature = "cuda-verifier")]
    assert!(defered_verifier.fast_check_gpu());
    #[cfg(not(feature = "cuda-verifier"))]
    assert!(defered_verifier.fast_check());

    let deferred_duration = start.elapsed();
    let deferred_duration_per_line =
        deferred_duration / BLOB_ROW_ENCODED as u32 / BATCHES as u32;

    info!(
        "Deferred verifier elapsed {:?} + {:?} = {:?} per line",
        instant_duration,
        deferred_duration_per_line,
        instant_duration + deferred_duration_per_line
    );

    info!("Total deferred duration {:?}", deferred_duration);
}

fn make_slices(
    params: &ZgEncoderParams,
) -> (G1Curve, [u8; 32], Vec<EncodedSlice>) {
    let mut data = vec![0u8; MAX_RAW_DATA_SIZE];
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
