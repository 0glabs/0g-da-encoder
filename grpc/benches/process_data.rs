use amt::ec_algebra::Fr;
use grpc::EncoderService;
use rand::thread_rng;
use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use amt::ec_algebra::UniformRand;
use ark_bn254::Bn254;
use rand::Rng;
use zg_encoder::{
    constants::{MAX_RAW_DATA_SIZE, RAW_BLOB_SIZE},
    ZgEncoderParams,
};

use tracing::{info, Level};

fn random_scalars(length: usize) -> Vec<Fr<Bn254>> {
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| Fr::<Bn254>::rand(&mut rng))
        .collect::<Vec<_>>()
}

fn init_logger() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_target(false)
        .init();
}

fn main() {
    init_logger();

    info!(current_dir = ?std::env::current_dir(), "Start");

    let start = Instant::now();

    let params = ZgEncoderParams::from_dir_mont("../crates/amt/pp", true, None);
    info!("Load time elapsed {:?}", start.elapsed());

    params.warmup();

    sleep(Duration::from_secs(1));

    // bench_amt(params);
    bench_all(params);
}

#[allow(dead_code)]
fn bench_amt(params: ZgEncoderParams) {
    let input = random_scalars(RAW_BLOB_SIZE);

    let start = Instant::now();
    for _ in 0..1 {
        let output = params.process_blob(&input[..]);
        std::hint::black_box(output);
    }
    info!(time = ?start.elapsed(), "Time elapsed");
}

fn bench_all(params: ZgEncoderParams) {
    let encoder = EncoderService { params };
    let mut data = vec![0u8; MAX_RAW_DATA_SIZE];
    thread_rng().fill(&mut data[..]);

    let start = Instant::now();
    for _ in 0..10 {
        let reply = encoder.process_data(&data, true).unwrap();
        std::hint::black_box(reply);
    }
    info!(time = ?start.elapsed(), "Time elapsed");
}
