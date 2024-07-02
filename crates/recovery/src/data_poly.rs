#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::collections::{BTreeMap, BTreeSet};

use ark_ff::{batch_inversion, Field, One, Zero};
use ark_std::{cfg_iter, rand, UniformRand};
use zg_encoder::constants::{
    Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N, RAW_BLOB_SIZE,
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

fn check_input(data: &BTreeMap<usize, Vec<Scalar>>) -> Result<(), RecoveryErr> {
    if data.len() < BLOB_ROW_N {
        return Err(RecoveryErr::TooFewRowIds);
    }

    // unwrap safety: data.len() >= BLOB_ROW_N = (1 << BLOB_ROW_LOG) >= 1, thus,
    // !data.is_empty() is always true
    if data.last_key_value().unwrap().0 >= &BLOB_ROW_ENCODED {
        return Err(RecoveryErr::RowIdOverflow);
    }

    for row_data in data.values() {
        if row_data.len() != BLOB_COL_N {
            return Err(RecoveryErr::InvalidLength);
        }
    }

    Ok(())
}

fn convert_input(
    data: &BTreeMap<usize, Vec<Scalar>>,
) -> (BTreeSet<usize>, Vec<Scalar>) {
    let erasured_row_ids: BTreeSet<usize> = (0..COSET_MORE * BLOB_ROW_N)
        .filter(|x| data.get(x).is_none())
        .collect();
    let mut data_times_z = vec![Scalar::zero(); COSET_MORE * RAW_BLOB_SIZE];
    for (row_idx, row_data) in data {
        for (elem_idx, elem_data) in data_times_z
            .iter_mut()
            .take((row_idx + 1) * BLOB_COL_N)
            .skip(row_idx * BLOB_COL_N)
            .enumerate()
        {
            *elem_data = row_data[elem_idx]
        }
    }
    (erasured_row_ids, data_times_z)
}

pub fn data_poly(
    data: &BTreeMap<usize, Vec<Scalar>>,
) -> Result<Vec<Scalar>, RecoveryErr> {
    check_input(data)?;
    let (erasured_row_ids, erasured_data) = convert_input(data);
    const TRY_TIMES: usize = 100;

    let zcoeffs = zpoly(&erasured_row_ids).to_vec();

    let data_times_zcoeffs =
        data_times_zpoly(&erasured_row_ids, &erasured_data, &zcoeffs).to_vec();

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
        let data_kx_coeffs = evals_to_poly(&data_kx_evals).to_vec();
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
            coeffs_to_evals_larger, evals_to_poly, random_row_ids,
            random_scalars,
        },
        zpoly::COSET_MORE,
    };
    use amt::{change_matrix_direction, to_coset_blob};
    use ark_ff::Zero;
    use ark_std::rand::{thread_rng, Rng};
    use std::collections::BTreeMap;
    use zg_encoder::constants::{
        Scalar, BLOB_COL_LOG, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_LOG,
        BLOB_ROW_N, COSET_N, ENCODED_BLOB_SIZE, PE, RAW_BLOB_SIZE,
    };

    fn get_data_poly(
        row_ids: &[usize], data_before_erasured: &[Scalar],
    ) -> Result<Vec<Scalar>, RecoveryErr> {
        let data: BTreeMap<usize, Vec<Scalar>> = row_ids
            .iter()
            .map(|row_idx| {
                (
                    *row_idx,
                    data_before_erasured[std::cmp::min(
                        row_idx,
                        &(BLOB_ROW_ENCODED - 1),
                    ) * BLOB_COL_N
                        ..std::cmp::min(row_idx + 1, BLOB_ROW_ENCODED)
                            * BLOB_COL_N]
                        .to_vec(),
                )
            })
            .collect();
        data_poly(&data)
    }

    fn check_data_poly(row_ids: &[usize], data_before_erasured: &[Scalar]) {
        let evals = get_data_poly(row_ids, data_before_erasured).unwrap();
        assert_eq!(data_before_erasured, evals);
    }

    fn random_data_before_erasured<R: Rng>(
        coset_num: usize, rng: &mut R,
    ) -> Vec<Scalar> {
        let mut data = random_scalars(RAW_BLOB_SIZE, rng);
        change_matrix_direction(&mut data, BLOB_COL_LOG, BLOB_ROW_LOG);
        let mut data_before_erasured_chunks: Vec<_> = (0..coset_num)
            .map(|coset_idx| to_coset_blob::<PE>(&data, coset_idx))
            .collect();
        for chunk in data_before_erasured_chunks.iter_mut() {
            change_matrix_direction(chunk, BLOB_ROW_LOG, BLOB_COL_LOG);
        }
        let data_before_erasured = data_before_erasured_chunks
            .into_iter()
            .flat_map(|x| x)
            .collect();
        data_before_erasured
    }

    fn test_data_poly_with_data<R: Rng>(
        data_before_erasured: &[Scalar], rng: &mut R,
    ) {
        let row_ids: Vec<usize> = (1..BLOB_ROW_ENCODED).collect();
        check_data_poly(&row_ids, data_before_erasured);
        let mut row_ids: Vec<usize> = (0..BLOB_ROW_N).collect();
        row_ids.extend((BLOB_ROW_N + 1)..BLOB_ROW_ENCODED);
        check_data_poly(&row_ids, data_before_erasured);
        let row_ids: Vec<usize> =
            ((BLOB_ROW_N + 1)..BLOB_ROW_ENCODED).collect();
        check_data_poly(&row_ids, data_before_erasured);
        let row_ids: Vec<usize> =
            ((BLOB_ROW_ENCODED - BLOB_ROW_N)..BLOB_ROW_ENCODED).collect();
        check_data_poly(&row_ids, data_before_erasured);
        let row_ids: Vec<usize> = (0..BLOB_ROW_N).collect();
        check_data_poly(&row_ids, data_before_erasured);
        let row_ids: Vec<usize> =
            (0..BLOB_ROW_ENCODED).skip(1).step_by(2).collect();
        check_data_poly(&row_ids, data_before_erasured);

        let row_ids: Vec<usize> =
            (BLOB_ROW_ENCODED - BLOB_ROW_N + 2..BLOB_ROW_ENCODED + 1).collect();
        assert_eq!(
            get_data_poly(&row_ids, data_before_erasured),
            Err(RecoveryErr::TooFewRowIds)
        );
        let row_ids: Vec<usize> =
            (BLOB_ROW_ENCODED - BLOB_ROW_N + 1..BLOB_ROW_ENCODED + 1).collect();
        assert_eq!(
            get_data_poly(&row_ids, data_before_erasured),
            Err(RecoveryErr::RowIdOverflow)
        );

        for _ in 0..3 {
            for row_num in BLOB_ROW_N..BLOB_ROW_ENCODED + 1 {
                let row_ids = random_row_ids(row_num, rng);
                check_data_poly(&row_ids, data_before_erasured);
            }
        }
    }

    #[test]
    fn test_data_poly() {
        let mut rng = thread_rng();
        test_data_poly_with_data(
            &vec![Scalar::zero(); ENCODED_BLOB_SIZE],
            &mut rng,
        );
        println!("zero test is Ok");
        test_data_poly_with_data(
            &random_data_before_erasured(COSET_N, &mut rng),
            &mut rng,
        );
        println!("random test is Ok");
    }

    fn check_evals_to_poly(data_before_recovery: &[Scalar]) {
        let coeffs = evals_to_poly(data_before_recovery).to_vec();
        assert!(coeffs.len() <= RAW_BLOB_SIZE);
        let evals_larger = coeffs_to_evals_larger(&coeffs);
        assert_eq!(evals_larger, data_before_recovery);
        assert!(coeffs.len() <= COSET_MORE * RAW_BLOB_SIZE);
    }

    #[test]
    fn test_evals_to_poly() {
        let mut rng = thread_rng();
        check_evals_to_poly(&vec![Scalar::zero(); COSET_MORE * RAW_BLOB_SIZE]);
        check_evals_to_poly(&random_data_before_erasured(COSET_MORE, &mut rng));
    }
}
