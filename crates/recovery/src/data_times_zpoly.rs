use std::collections::BTreeSet;

use zg_encoder::constants::{Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, ENCODED_BLOB_SIZE};
use ark_ff::Zero;

pub fn data_times_zpoly(line_ids: BTreeSet<usize>, data_before_recovery: Vec<Scalar>) -> Vec<Scalar> {
    if !line_ids.is_empty() {
        assert!(line_ids.last().unwrap() < &BLOB_ROW_ENCODED);
    }
    assert_eq!(data_before_recovery.len(), ENCODED_BLOB_SIZE);
    let mut data_times_z = data_before_recovery.clone();
    for row_idx in &line_ids {
        for idx in (row_idx * BLOB_COL_N)..((row_idx + 1) * BLOB_COL_N) {
            data_times_z[idx] = Scalar::zero();
        }
    }
    let zeros_ids: Vec<_> = line_ids
        .iter()
        .flat_map(|idx| (idx * BLOB_COL_N)..(idx + 1) * BLOB_COL_N)
        .collect();
    data_times_z[zeros_ids] = Scalar::zero();
    let mut polys = vec![Poly::One(()); BLOB_ROW_ENCODED.next_power_of_two()];
    let zblob = ZBlob::init();
    for line_id in line_ids.iter() {
        let mut sparse = BTreeMap::new();
        sparse.insert(BLOB_COL_N, Scalar::one());
        let coset_idx = line_id / BLOB_ROW_N;
        let local_idx = line_id % BLOB_ROW_N;
        sparse.insert(0, zblob.get_item(coset_idx, local_idx));
        polys[*line_id] = Poly::Sparse(sparse);
    }
    let num_iter = log2(polys.len()) as usize;
    for _ in 0..num_iter {
        polys = polys
            .chunks_exact(2)
            .map(|x| x[0].multiply(&x[1]))
            .collect::<Vec<_>>();
    }
    assert_eq!(polys.len(), 1);
    polys[0].to_vec()
}
