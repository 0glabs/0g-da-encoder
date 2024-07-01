#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::collections::BTreeSet;

use ark_ff::{batch_inversion, Field, One, Zero};
use ark_std::{cfg_iter, rand, UniformRand};
use zg_encoder::constants::{
    Scalar, BLOB_ROW_ENCODED, BLOB_ROW_N, ENCODED_BLOB_SIZE, RAW_BLOB_SIZE,
};

use crate::{
    data_times_zpoly::data_times_zpoly,
    error::RecoveryErr,
    utils::{
        coeffs_to_evals, coeffs_to_evals_larger, evals_to_poly, fx_to_fkx,
    },
    zpoly::{zpoly, COSET_MORE},
};

fn inverse_vec_checked(input: &mut [Scalar]) -> bool {
    if cfg_iter!(input, 65536).any(|x| x.is_zero()) {
        return false;
    }
    batch_inversion(input);
    true
}

fn check_input(
    row_ids: &BTreeSet<usize>, data_before_recovery: &[Scalar],
) -> Result<(), RecoveryErr> {
    if data_before_recovery.len() != ENCODED_BLOB_SIZE {
        return Err(RecoveryErr::InvalidLength);
    }
    if !row_ids.is_empty() && row_ids.last().unwrap() >= &BLOB_ROW_ENCODED {
        return Err(RecoveryErr::RowIdOverflow);
    }
    if row_ids.len() > BLOB_ROW_ENCODED - BLOB_ROW_N {
        return Err(RecoveryErr::TooManyRowIds);
    }
    Ok(())
}

pub fn data_poly(
    row_ids: &BTreeSet<usize>, data_before_recovery: &[Scalar],
) -> Result<Vec<Scalar>, RecoveryErr> {
    check_input(row_ids, data_before_recovery)?;
    const TRY_TIMES: usize = 100;

    let zcoeffs = zpoly(row_ids).to_vec();

    let data_times_zcoeffs =
        data_times_zpoly(row_ids, data_before_recovery, &zcoeffs).to_vec();

    assert!(zcoeffs.len() <= COSET_MORE * RAW_BLOB_SIZE);
    assert!(data_times_zcoeffs.len() <= COSET_MORE * RAW_BLOB_SIZE);

    let mut rng = rand::thread_rng();
    for _ in 0..TRY_TIMES {
        let k = Scalar::rand(&mut rng);
        if k == Scalar::zero() || k == Scalar::one() {
            continue;
        }
        let k_inverse = k.inverse().unwrap();

        let zcoeffs_kx = fx_to_fkx(&zcoeffs, k);
        let mut z_kx_evals = coeffs_to_evals_larger(&zcoeffs_kx);
        let success = inverse_vec_checked(&mut z_kx_evals);
        if !success {
            continue;
        }

        let z_kx_evals_inverse = z_kx_evals;

        let data_times_zcoeffs_kx = fx_to_fkx(&data_times_zcoeffs, k);
        let data_times_z_kx_evals =
            coeffs_to_evals_larger(&data_times_zcoeffs_kx);
        let data_kx_evals: Vec<_> = cfg_iter!(data_times_z_kx_evals)
            .zip(cfg_iter!(z_kx_evals_inverse))
            .map(|(x, y)| x * y)
            .collect();
        let data_kx_coeffs = evals_to_poly(data_kx_evals).to_vec();
        let data_coeffs = fx_to_fkx(&data_kx_coeffs, k_inverse);
        assert!(data_coeffs.len() <= RAW_BLOB_SIZE + 1);

        return Ok(coeffs_to_evals(&data_coeffs));
    }
    Err(RecoveryErr::ExtaustiveK)
}

#[cfg(test)]
mod tests {
    use crate::{
        data_poly::data_poly,
        error::RecoveryErr,
        utils::{
            coeffs_to_evals, coeffs_to_evals_larger, coeffs_to_evals_more,
            evals_to_poly, random_scalars,
        },
        zpoly::COSET_MORE,
    };
    use amt::{change_matrix_direction, to_coset_blob};
    use ark_ff::Zero;
    use std::collections::BTreeSet;
    use zg_encoder::constants::{
        Scalar, BLOB_COL_LOG, BLOB_ROW_ENCODED, BLOB_ROW_LOG, BLOB_ROW_N,
        COSET_N, ENCODED_BLOB_SIZE, PE, RAW_BLOB_SIZE,
    };

    fn check_data_poly(
        row_ids: BTreeSet<usize>, data_before_recovery: &[Scalar],
    ) {
        let evals = data_poly(&row_ids, data_before_recovery).unwrap();

        assert_eq!(data_before_recovery, evals);
    }

    fn random_data_before_recovery(coset_num: usize) -> Vec<Scalar> {
        let mut data = random_scalars(RAW_BLOB_SIZE);
        change_matrix_direction(&mut data, BLOB_COL_LOG, BLOB_ROW_LOG);
        let mut data_before_recovery_chunks: Vec<_> = (0..coset_num)
            .map(|coset_idx| to_coset_blob::<PE>(&data, coset_idx))
            .collect();
        for chunk in data_before_recovery_chunks.iter_mut() {
            change_matrix_direction(chunk, BLOB_ROW_LOG, BLOB_COL_LOG);
        }
        let data_before_recovery = data_before_recovery_chunks
            .into_iter()
            .flat_map(|x| x)
            .collect();
        data_before_recovery
    }

    fn test_data_poly_with_data(data_before_recovery: Vec<Scalar>) {
        check_data_poly(BTreeSet::from([0]), &data_before_recovery);
        check_data_poly(
            BTreeSet::from([BLOB_ROW_N + 1, BLOB_ROW_N * 2]),
            &data_before_recovery,
        );
        check_data_poly(
            BTreeSet::from([BLOB_ROW_N, BLOB_ROW_N + 1]),
            &data_before_recovery,
        );
        let mut all: BTreeSet<usize> = (0..BLOB_ROW_N).collect();
        check_data_poly(all.clone(), &data_before_recovery);
        all = (0..BLOB_ROW_N * 2).collect();
        check_data_poly(all.clone(), &data_before_recovery);
        all = (BLOB_ROW_N..BLOB_ROW_ENCODED).collect();
        check_data_poly(all.clone(), &data_before_recovery);
        all.remove(&(BLOB_ROW_N * 2 - 1));
        check_data_poly(all, &data_before_recovery);
        all = (0..BLOB_ROW_ENCODED).step_by(2).collect();
        check_data_poly(all, &data_before_recovery);
        all = (0..BLOB_ROW_N * 2 + 1).collect();
        assert_eq!(
            data_poly(&all, &data_before_recovery),
            Err(RecoveryErr::TooManyRowIds)
        );
        all = (BLOB_ROW_N..BLOB_ROW_ENCODED + 1).collect();
        assert_eq!(
            data_poly(&all, &data_before_recovery),
            Err(RecoveryErr::RowIdOverflow)
        );
    }

    #[test]
    fn test_data_poly() {
        test_data_poly_with_data(vec![Scalar::zero(); ENCODED_BLOB_SIZE]);
        println!("zero test is Ok");
        test_data_poly_with_data(random_data_before_recovery(COSET_N));
        println!("random test is Ok");
    }

    fn check_evals_to_poly(data_before_recovery: Vec<Scalar>) {
        let coeffs = evals_to_poly(data_before_recovery.clone()).to_vec();
        assert!(coeffs.len() <= RAW_BLOB_SIZE);
        let evals_larger = coeffs_to_evals_larger(&coeffs);
        assert_eq!(evals_larger, data_before_recovery);
        assert!(coeffs.len() <= COSET_MORE * RAW_BLOB_SIZE);
        let evals = coeffs_to_evals(&coeffs);
        assert_eq!(evals, evals_larger[..ENCODED_BLOB_SIZE]);
        let evals_more = coeffs_to_evals_more(&coeffs);
        assert_eq!(evals_more, evals_larger[ENCODED_BLOB_SIZE..]);
    }

    #[test]
    fn test_evals_to_poly() {
        check_evals_to_poly(vec![Scalar::zero(); COSET_MORE * RAW_BLOB_SIZE]);
        check_evals_to_poly(random_data_before_recovery(COSET_MORE));
    }
}
