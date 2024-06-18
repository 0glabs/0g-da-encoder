use std::cmp::max;

use ark_std::cfg_chunks_mut;
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use zg_encoder::constants::ENCODED_BLOB_SIZE;
use zg_encoder::constants::{Scalar, BLOB_COL_LOG, BLOB_ROW_LOG, COSET_N, PE, RAW_BLOB_SIZE};
use amt::AMTParams;
use amt::change_matrix_direction;
use ark_ff::{Zero, Field};
use ark_std::{rand, UniformRand};

use crate::poly::Poly;

const SPARSE_THRES: usize = 100;
pub fn many_non_zeros(vec: &[Scalar]) -> bool {
    let mut num_non_zeros = 0;
    for scalar in vec {
        if *scalar != Scalar::zero() {
            num_non_zeros += 1;
            if num_non_zeros > SPARSE_THRES {
                return true
            }
        }
    }
    false
}

pub fn random_scalars(length: usize) -> Vec<Scalar> {
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| Scalar::rand(&mut rng))
        .collect::<Vec<_>>()
}

fn coeffs_to_evals_coset(mut coeffs: Vec<Scalar>, coset_idx: usize) -> Vec<Scalar> {

    let coset_w = AMTParams::<PE>::coset_factor(RAW_BLOB_SIZE, coset_idx);
    if coset_idx != 0 {
        cfg_chunks_mut!(coeffs, 16)
            .enumerate()
            .for_each(|(idx, chunks)| {
                let mut base = coset_w.pow([idx as u64 * 16]);
                for x in chunks.iter_mut() {
                    *x *= base;
                    base *= coset_w;
                }
            });
    }

    assert!(coeffs.len() <= ENCODED_BLOB_SIZE + 1);
    assert!(RAW_BLOB_SIZE.is_power_of_two());
    let fft_degree = max(coeffs.len().next_power_of_two(), RAW_BLOB_SIZE);
    let fft_domain = Radix2EvaluationDomain::<Scalar>::new(fft_degree).unwrap();
    let mut evals = fft_domain.fft(&coeffs);
    if fft_degree > RAW_BLOB_SIZE {
        evals = evals.into_iter().step_by(fft_degree / RAW_BLOB_SIZE).collect();
    }
    change_matrix_direction(&mut evals, BLOB_ROW_LOG, BLOB_COL_LOG);
    evals
}

pub fn coeffs_to_evals(coeffs: Vec<Scalar>) -> Vec<Scalar> {
    (0..COSET_N).flat_map(|coset_idx| coeffs_to_evals_coset(coeffs.clone(), coset_idx)).collect()
}

pub fn ifft_allow_all_zeros(evals: &[Scalar], fft_degree: usize, res_degree: usize,
    fft_domain: Radix2EvaluationDomain::<Scalar>
) -> Poly {
    dbg!(evals.len(), fft_degree, res_degree);
    if evals.iter().all(|&x| x == Scalar::zero()) {
        assert_eq!(evals.len(), fft_degree);
        panic!();
        Poly::poly_all_zeros(fft_degree)
    }
    else {
        let coeffs = fft_domain.ifft(&evals);
        let all_zeros = coeffs[(res_degree + 1)..].iter().all(|&x| x == Scalar::zero());
        assert!(all_zeros);
        assert_ne!(coeffs[res_degree], Scalar::zero());
        Poly::from_vec_uncheck(coeffs[..(res_degree + 1)].to_vec())
    }
}

fn evals_to_poly_coset(mut evals: Vec<Scalar>, coset_idx: usize) -> Poly {
    assert_eq!(evals.len(), RAW_BLOB_SIZE);
    change_matrix_direction(&mut evals, BLOB_COL_LOG, BLOB_ROW_LOG);
    let res_degree = RAW_BLOB_SIZE;
    let fft_degree = (res_degree + 1).next_power_of_two();
    let fft_domain = Radix2EvaluationDomain::<Scalar>::new(fft_degree).unwrap();
    let poly = ifft_allow_all_zeros(&evals, fft_degree, res_degree, fft_domain);

    
    let coset_w = AMTParams::<PE>::coset_factor(RAW_BLOB_SIZE, coset_idx);
    if coset_idx != 0 {
        cfg_chunks_mut!(coeffs, 16)
            .enumerate()
            .for_each(|(idx, chunks)| {
                let mut base = coset_w.pow([idx as u64 * 16]);
                for x in chunks.iter_mut() {
                    *x *= base;
                    base *= coset_w;
                }
            });
    }

    assert!(coeffs.len() <= ENCODED_BLOB_SIZE + 1);
    assert!(RAW_BLOB_SIZE.is_power_of_two());
    let fft_degree = max(coeffs.len().next_power_of_two(), RAW_BLOB_SIZE);
    
    let mut evals = fft_domain.fft(&coeffs);
    if fft_degree > RAW_BLOB_SIZE {
        evals = evals.into_iter().step_by(fft_degree / RAW_BLOB_SIZE).collect();
    }
    
    evals
}