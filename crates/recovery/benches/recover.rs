#[macro_use]
extern crate tracing;

#[macro_use]
extern crate ark_std;

use std::time::Instant;

use rand::{seq::index::sample, thread_rng, Rng};
use tracing::Level;
use zg_da_recovery::recover_from_da_slice;

use zg_encoder::{
    constants::{BLOB_ROW_ENCODED, BLOB_ROW_N, MAX_RAW_DATA_SIZE},
    EncodedBlob, RawBlob, RawData, ZgEncoderParams,
};

fn init_logger() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_target(false)
        .init();
}

fn main() {
    init_logger();

    info!("load params");
    let param_dir = "../amt/pp";
    let params = ZgEncoderParams::from_dir_mont(param_dir, true, None);

    let mut rng = thread_rng();

    let mut data = vec![0u8; MAX_RAW_DATA_SIZE];
    rng.fill(&mut data[..]);
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.into();

    info!("encode");
    let encoded_blob = EncodedBlob::build(&raw_blob, &params);

    info!("build lines");
    let lines: Vec<Vec<u8>> = (0..BLOB_ROW_ENCODED)
        .map(|i| {
            encoded_blob
                .get_row(i)
                .merkle_row()
                .into_iter()
                .flat_map(IntoIterator::into_iter)
                .collect::<Vec<u8>>()
        })
        .collect();

    info!("bench");

    for valid_percent in (100..=300).step_by(10) {
        let num_lines = BLOB_ROW_N * valid_percent / 100;
        let filtered = sample(&mut rng, BLOB_ROW_ENCODED, num_lines)
            .iter()
            .map(|i| (i, lines[i].clone()))
            .collect();

        let start = Instant::now();
        let output = recover_from_da_slice(&filtered).unwrap();
        assert_eq!(output, data);
        info!(dur = ?start.elapsed(), num_lines, "Time elapsed");
    }
}
