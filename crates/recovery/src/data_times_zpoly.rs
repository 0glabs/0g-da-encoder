use ark_std::cfg_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::collections::BTreeSet;

use zg_encoder::constants::{Scalar, BLOB_ROW_N, RAW_BLOB_SIZE};

use crate::{
    poly::Poly,
    utils::{coeffs_to_evals_larger, evals_to_poly},
    zpoly::COSET_MORE,
};

pub fn data_times_zpoly(
    erasured_row_ids: &BTreeSet<usize>, erasured_data: &[Scalar],
    zcoeffs: &[Scalar],
) -> Poly {
    if !erasured_row_ids.is_empty() {
        assert!(*erasured_row_ids.last().unwrap() < COSET_MORE * BLOB_ROW_N);
    }
    assert_eq!(erasured_data.len(), COSET_MORE * RAW_BLOB_SIZE);

    assert!(zcoeffs.len() <= COSET_MORE * RAW_BLOB_SIZE + 1);
    let zevals = coeffs_to_evals_larger(zcoeffs);
    let data_times_z: Vec<Scalar> = cfg_iter!(erasured_data)
        .zip(cfg_iter!(zevals))
        .map(|(x, y)| x * y)
        .collect();

    evals_to_poly(&data_times_z)
}

#[cfg(test)]
mod tests {
    use crate::{
        data_times_zpoly::data_times_zpoly,
        utils::{coeffs_to_evals_larger, random_scalars},
        zpoly::{zpoly, COSET_MORE},
    };
    use ark_ff::Zero;
    use rand::thread_rng;
    use std::collections::BTreeSet;
    use zg_encoder::constants::{
        Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N, COSET_N,
        ENCODED_BLOB_SIZE, RAW_BLOB_SIZE,
    };

    fn check_data_times_zpoly(
        row_ids: &mut BTreeSet<usize>, data_before_recovery: &[Scalar],
    ) {
        let mut erasured_data = data_before_recovery.to_vec();
        for row_idx in row_ids.iter() {
            for erasured in erasured_data
                .iter_mut()
                .take((row_idx + 1) * BLOB_COL_N)
                .skip(row_idx * BLOB_COL_N)
            {
                *erasured = Scalar::zero();
            }
        }
        erasured_data.extend(
            std::iter::repeat(Scalar::zero())
                .take((COSET_MORE - COSET_N) * RAW_BLOB_SIZE),
        );

        row_ids.extend(BLOB_ROW_ENCODED..COSET_MORE * BLOB_ROW_N);
        let zcoeffs = zpoly(&row_ids).to_vec();

        let coeffs =
            data_times_zpoly(&row_ids, &erasured_data, &zcoeffs).to_vec();
        assert!(coeffs.len() <= COSET_MORE * RAW_BLOB_SIZE);

        let evals = coeffs_to_evals_larger(&coeffs);
        for row_idx in row_ids.iter() {
            for idx in (row_idx * BLOB_COL_N)..((row_idx + 1) * BLOB_COL_N) {
                assert_eq!(evals[idx], Scalar::zero());
            }
        }
    }

    #[test]
    fn test_data_times_zpoly() {
        let mut rng = thread_rng();
        let data_before_recovery = random_scalars(ENCODED_BLOB_SIZE, &mut rng);
        check_data_times_zpoly(&mut BTreeSet::from([0]), &data_before_recovery);
        check_data_times_zpoly(
            &mut BTreeSet::from([BLOB_ROW_N, BLOB_ROW_N + 1]),
            &data_before_recovery,
        );
        let mut all: BTreeSet<usize> = (0..BLOB_ROW_N).collect();
        check_data_times_zpoly(&mut all, &data_before_recovery);
        all = (0..BLOB_ROW_N * 2).collect();
        check_data_times_zpoly(&mut all, &data_before_recovery);
        all = (0..BLOB_ROW_ENCODED).collect();
        check_data_times_zpoly(&mut all, &data_before_recovery);
        all.remove(&(BLOB_ROW_N * 2 - 1));
        check_data_times_zpoly(&mut all, &data_before_recovery);
        all = (0..BLOB_ROW_ENCODED).step_by(1).collect();
        check_data_times_zpoly(&mut all, &data_before_recovery);
    }
}
