use std::collections::BTreeSet;

use amt::change_matrix_direction;
use ark_ff::{Field, One, Zero};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::{rand, UniformRand};
use zg_encoder::constants::{Scalar, BLOB_COL_LOG, BLOB_COL_N, BLOB_ROW_LOG, COSET_N, RAW_BLOB_SIZE};

use crate::{
    data_times_zpoly::data_times_zpoly, error::RecoveryErr, poly::Poly, utils::{
        coeffs_to_evals, coeffs_to_evals_larger, evals_to_poly, fx_to_fkx,
    }, zpoly::{self, zpoly, COSET_MORE}
};

const TRY_TIMES: usize = 1000;

pub fn inverse_vec(vec: Vec<Scalar>) -> Option<Vec<Scalar>> {
    vec.iter().map(|x| x.inverse()).collect()
}

pub fn data_poly(
    line_ids: BTreeSet<usize>, data_before_recovery: &[Scalar],
) -> Result<Vec<Scalar>, RecoveryErr> {
    let zcoeffs = zpoly(line_ids.clone()).to_vec();
    let data_times_zcoeffs = data_times_zpoly(line_ids, data_before_recovery, &zcoeffs).to_vec();
    
    let mut rng = rand::thread_rng();
    for _ in 0..TRY_TIMES {
        let k = Scalar::rand(&mut rng);
        if k == Scalar::zero() || k == Scalar::one() {
            continue;
        }
        if let Some(k_inverse) = k.inverse() {
            let zcoeffs_kx = fx_to_fkx(&zcoeffs, k);
            let z_kx_evals = coeffs_to_evals_larger(&zcoeffs_kx);
            if let Some(z_kx_evals_inverse) = inverse_vec(z_kx_evals) {
                let data_times_zcoeffs_kx = fx_to_fkx(&data_times_zcoeffs, k);
                let data_times_z_kx_evals =
                    coeffs_to_evals_larger(&data_times_zcoeffs_kx);
                let data_kx_evals: Vec<_> = data_times_z_kx_evals
                    .iter()
                    .zip(z_kx_evals_inverse.iter())
                    .map(|(x, y)| x * y)
                    .collect();
                let data_kx_coeffs = evals_to_poly(data_kx_evals).to_vec();
                let data_coeffs = fx_to_fkx(&data_kx_coeffs, k_inverse);
                assert!(data_coeffs.len() <= RAW_BLOB_SIZE + 1);
                return Ok(coeffs_to_evals(&data_coeffs));
            }
        }
    }
    Err(RecoveryErr::ExtaustiveK)
}

#[cfg(test)]
mod tests {
    use crate::data_poly::data_poly;
    use crate::data_times_zpoly::data_times_zpoly;
    use crate::utils::{
        coeffs_to_evals, coeffs_to_evals_larger, coeffs_to_evals_more,
        evals_to_poly, random_scalars,
    };
    use crate::zpoly::{zpoly, COSET_MORE};
    use amt::{change_matrix_direction, to_coset_blob};
    use ark_ff::Zero;
    use std::collections::BTreeSet;
    use zg_encoder::constants::{
        Scalar, BLOB_COL_LOG, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_LOG,
        BLOB_ROW_N, COSET_N, ENCODED_BLOB_SIZE, PE, RAW_BLOB_SIZE,
    };

    fn check_data_poly(
        line_ids: BTreeSet<usize>, data_before_recovery: &[Scalar],
    ) {
        let evals = data_poly(line_ids, data_before_recovery).unwrap();

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
    }

    #[test]
    fn test_data_poly() {
        test_data_poly_with_data(vec![Scalar::zero(); ENCODED_BLOB_SIZE]);
        dbg!("zero test is Ok");
        test_data_poly_with_data(random_data_before_recovery(COSET_N));
        dbg!("random test is Ok");
    }

    fn check_evals_to_poly(data_before_recovery: Vec<Scalar>) {
        let coeffs = evals_to_poly(data_before_recovery.clone()).to_vec();
        assert!(coeffs.len() <= RAW_BLOB_SIZE);
        let evals_larger = coeffs_to_evals_larger(&coeffs);
        assert_eq!(evals_larger, data_before_recovery);
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
