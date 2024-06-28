use amt::ec_algebra::{AffineRepr, CanonicalDeserialize};
use ark_bn254::{Fq, G1Affine};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use grpc::{EncoderService, SignerService};
use rand::{rngs::StdRng, Rng, SeedableRng};
use zg_encoder::{
    constants::{G1Curve, BLOB_COL_N, BLOB_ROW_N, RAW_UNIT},
    EncodedSlice,
};

fn signer(
    encoded_slice: &EncodedSlice, signer_service: &SignerService,
    authoritative_commitment: &G1Curve, authoritative_root: &[u8; 32],
) {
    encoded_slice
        .verify(
            &signer_service.params,
            &authoritative_commitment,
            &authoritative_root,
            None,
        )
        .unwrap()
}

fn criterion_benchmark(c: &mut Criterion) {
    let param_dir = "../crates/amt/pp";
    let encoder_service = EncoderService::new_for_test(param_dir);
    let num_bytes = RAW_UNIT * BLOB_ROW_N * BLOB_COL_N;
    // generate input
    let seed = 222u64;
    let mut rng = StdRng::seed_from_u64(seed);
    let mut data = vec![0u8; num_bytes];
    rng.fill(&mut data[..]);
    let reply = encoder_service.process_data(&data, true).unwrap();
    let erasure_commitment = {
        let mut raw_commitment = &*reply.erasure_commitment;
        let x = Fq::deserialize_uncompressed(&mut raw_commitment).unwrap();
        let y = Fq::deserialize_uncompressed(&mut raw_commitment).unwrap();
        G1Affine::new(x, y).into_group()
    };
    let storage_root =
        <[u8; 32]>::deserialize_uncompressed(&*reply.storage_root).unwrap();
    let encoded_slice: Vec<_> = reply
        .encoded_slice
        .iter()
        .map(|row| {
            EncodedSlice::deserialize_uncompressed(&*row.to_vec()).unwrap()
        })
        .collect();
    let num_slice = encoded_slice.len();

    let signer_service = SignerService::new(param_dir);

    let mut group = c.benchmark_group("signer");
    for i in [0usize, 3, 10, 37, num_slice - 1] {
        group.bench_function(i.to_string(), |b| {
            b.iter(|| {
                signer(
                    black_box(&encoded_slice[i]),
                    black_box(&signer_service),
                    black_box(&erasure_commitment),
                    black_box(&storage_root),
                )
            })
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
