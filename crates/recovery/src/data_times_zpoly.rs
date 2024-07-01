use ark_std::cfg_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::collections::BTreeSet;

use ark_ff::Zero;
use zg_encoder::constants::{
    Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, COSET_N, ENCODED_BLOB_SIZE,
    RAW_BLOB_SIZE,
};

use crate::{
    poly::Poly,
    utils::{coeffs_to_evals_larger, evals_to_poly},
    zpoly::COSET_MORE,
};

pub fn data_times_zpoly(
    row_ids: &BTreeSet<usize>, data_before_recovery: &[Scalar],
    zcoeffs: &[Scalar],
) -> Poly {
    if !row_ids.is_empty() {
        assert!(row_ids.last().unwrap() < &BLOB_ROW_ENCODED);
    }
    assert_eq!(data_before_recovery.len(), ENCODED_BLOB_SIZE);
    let mut data_times_z = data_before_recovery.to_vec();
    for row_idx in row_ids {
        for erasured in data_times_z
            .iter_mut()
            .take((row_idx + 1) * BLOB_COL_N)
            .skip(row_idx * BLOB_COL_N)
        {
            *erasured = Scalar::zero();
        }
    }
    data_times_z.extend(
        std::iter::repeat(Scalar::zero())
            .take((COSET_MORE - COSET_N) * RAW_BLOB_SIZE),
    );

    assert!(zcoeffs.len() <= COSET_MORE * RAW_BLOB_SIZE + 1);
    let zevals = coeffs_to_evals_larger(zcoeffs);
    data_times_z = cfg_iter!(data_times_z)
        .zip(cfg_iter!(zevals))
        .map(|(x, y)| x * y)
        .collect();

    evals_to_poly(data_times_z)
}

#[cfg(test)]
mod tests {
    use crate::{
        data_times_zpoly::data_times_zpoly,
        utils::{coeffs_to_evals, coeffs_to_evals_more, random_scalars},
        zpoly::{zpoly, COSET_MORE},
    };
    use ark_ff::Zero;
    use std::collections::BTreeSet;
    use zg_encoder::constants::{
        Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N, ENCODED_BLOB_SIZE,
        RAW_BLOB_SIZE,
    };

    fn check_data_times_zpoly(
        row_ids: BTreeSet<usize>, data_before_recovery: &[Scalar],
    ) {
        let zcoeffs = zpoly(&row_ids).to_vec();
        let coeffs =
            data_times_zpoly(&row_ids, data_before_recovery, &zcoeffs).to_vec();
        assert!(coeffs.len() <= COSET_MORE * RAW_BLOB_SIZE);

        let evals = coeffs_to_evals(&coeffs);
        for row_idx in &row_ids {
            for idx in (row_idx * BLOB_COL_N)..((row_idx + 1) * BLOB_COL_N) {
                assert_eq!(evals[idx], Scalar::zero());
            }
        }

        let more_evals = coeffs_to_evals_more(&coeffs);
        assert!(more_evals.iter().all(|x| *x == Scalar::zero()));
    }

    #[test]
    fn test_data_times_zpoly() {
        let data_before_recovery = random_scalars(ENCODED_BLOB_SIZE);
        check_data_times_zpoly(BTreeSet::from([0]), &data_before_recovery);
        check_data_times_zpoly(
            BTreeSet::from([BLOB_ROW_N + 1, BLOB_ROW_N * 2]),
            &data_before_recovery,
        );
        check_data_times_zpoly(
            BTreeSet::from([BLOB_ROW_N, BLOB_ROW_N + 1]),
            &data_before_recovery,
        );
        let mut all: BTreeSet<usize> = (0..BLOB_ROW_N).collect();
        check_data_times_zpoly(all.clone(), &data_before_recovery);
        all = (0..BLOB_ROW_N * 2).collect();
        check_data_times_zpoly(all.clone(), &data_before_recovery);
        all = (0..BLOB_ROW_ENCODED).collect();
        check_data_times_zpoly(all.clone(), &data_before_recovery);
        all.remove(&(BLOB_ROW_N * 2 - 1));
        check_data_times_zpoly(all, &data_before_recovery);
        all = (0..BLOB_ROW_ENCODED).skip(1).collect();
        check_data_times_zpoly(all, &data_before_recovery);
    }
}
