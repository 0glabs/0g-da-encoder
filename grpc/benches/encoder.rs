use criterion::{black_box, criterion_group, criterion_main, Criterion};
use grpc::{EncodeBlobReply, EncoderService};
use rand::{rngs::StdRng, Rng, SeedableRng};
use zg_encoder::{
    constants::MAX_RAW_DATA_SIZE, scalar_to_h256, EncodedBlob, EncodedBlobAMT,
    EncodedBlobMerkle, RawBlob, RawData,
};

fn to_raw_data(data: &[u8]) -> RawData {
    let raw_data: RawData = data[..].try_into().unwrap();
    raw_data
}

fn to_raw_blob(data: &[u8]) -> RawBlob {
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.try_into().unwrap();
    raw_blob
}

fn to_encoded_amt(
    data: &[u8], encoder_service: &EncoderService,
) -> EncodedBlobAMT {
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.try_into().unwrap();
    let encoded_blob =
        EncodedBlobAMT::build(&raw_blob, &encoder_service.params);
    encoded_blob
}

fn to_blob_h256(data: &[u8]) -> Vec<[u8; 32]> {
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.try_into().unwrap();
    let double_blob: Vec<_> = [raw_blob.to_vec(), raw_blob.to_vec()].concat();
    let blob_h256: Vec<_> = double_blob
        .into_iter()
        .map(scalar_to_h256)
        .collect::<Vec<_>>();
    blob_h256
}

fn to_encoded_merkle(data: &[u8]) -> EncodedBlobMerkle {
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.try_into().unwrap();
    let double_blob: Vec<_> = [raw_blob.to_vec(), raw_blob.to_vec()].concat();
    let blob_h256: Vec<_> = double_blob
        .iter()
        .map(|x| scalar_to_h256(*x))
        .collect::<Vec<_>>();
    let encoded_blob = EncodedBlobMerkle::build(blob_h256);
    encoded_blob
}

fn to_encoded_blob(
    data: &[u8], encoder_service: &EncoderService,
) -> EncodedBlob {
    let raw_data: RawData = data[..].try_into().unwrap();
    let raw_blob: RawBlob = raw_data.into();
    let encoded_blob = EncodedBlob::build(&raw_blob, &encoder_service.params);
    encoded_blob
}

fn encoder(data: &[u8], encoder_service: &EncoderService) -> EncodeBlobReply {
    let reply = encoder_service.process_data(data, true).unwrap();
    reply
}

fn criterion_benchmark(c: &mut Criterion) {
    let encoder_service = EncoderService::new_for_test("../crates/amt/pp");
    // generate input
    let seed = 222u64;
    let mut rng = StdRng::seed_from_u64(seed);
    let mut data = vec![0u8; MAX_RAW_DATA_SIZE];
    rng.fill(&mut data[..]);

    let mut group = c.benchmark_group("sample_size");
    group.sample_size(10);
    group.bench_function("to_raw_data", |b| {
        b.iter(|| to_raw_data(black_box(&data)))
    });
    group.bench_function("to_raw_blob", |b| {
        b.iter(|| to_raw_blob(black_box(&data)))
    });
    group.bench_function("to_blob_h256", |b| {
        b.iter(|| to_blob_h256(black_box(&data)))
    });
    group.bench_function("to_encoded_merkle", |b| {
        b.iter(|| to_encoded_merkle(black_box(&data)))
    });
    group.bench_function("to_encoded_amt", |b| {
        b.iter(|| to_encoded_amt(black_box(&data), black_box(&encoder_service)))
    });
    group.bench_function("to_encoded_blob", |b| {
        b.iter(|| {
            to_encoded_blob(black_box(&data), black_box(&encoder_service))
        })
    });
    group.bench_function("encoder", |b| {
        b.iter(|| encoder(black_box(&data), black_box(&encoder_service)))
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
