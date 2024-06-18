use std::collections::{BTreeMap, BTreeSet};

use amt::AMTParams;
use zg_encoder::constants::{Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N, COSET_N, PE, RAW_BLOB_SIZE};
use ark_ff::{One, Field};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::log2;
use crate::poly::Poly;

// f_{blob_col_num} = 1
// f_0 = -C_i^{blob_col_num} = -coset_factor^{blob_col_num} * w^{blob_col_num}^i
// f_{others} = 0
// store f_0 for i = 0, 1, …, blob_row_num-1
#[derive(Debug)]
struct ZCoset(Vec<Scalar>);
struct ZBlob([ZCoset; COSET_N]);

impl ZCoset {
    fn init(idx: usize, w_power: Scalar) -> Self {
        let mut mul_factors = vec![w_power; BLOB_ROW_N];
        if idx == 0 {
            mul_factors[0] = -Scalar::one();
        }
        else {
            let coset_factor: Scalar = AMTParams::<PE>::coset_factor(RAW_BLOB_SIZE, idx);
            mul_factors[0] = -coset_factor.pow([BLOB_COL_N as u64]);
        }
        Self(mul_factors.iter()
            .scan(Scalar::one(), |state, x| {
                *state *= x;
                Some(*state)
            })
            .collect())
    }
}

impl ZBlob {
    fn init() -> Self {
        let fft_domain = Radix2EvaluationDomain::<Scalar>::new(RAW_BLOB_SIZE).unwrap();
        let root_of_unity = fft_domain.group_gen;
        let w_power: Scalar = root_of_unity.pow([BLOB_COL_N as u64]);
        let z_lines: Vec<ZCoset> = (0..COSET_N).map(|idx| ZCoset::init(idx, w_power)).collect();
        Self(z_lines.try_into().unwrap())
    }
    fn get_item(&self, coset_idx: usize, local_idx: usize) -> Scalar {
        self.0[coset_idx].0[local_idx]
    }
}

pub fn zpoly(line_ids: BTreeSet<usize>) -> Vec<Scalar> {
    if !line_ids.is_empty() {
        assert!(line_ids.last().unwrap() < &BLOB_ROW_ENCODED);
    }
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


#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use zg_encoder::constants::{Scalar, BLOB_COL_N, BLOB_ROW_ENCODED, BLOB_ROW_N};
    use ark_ff::Zero;
    use crate::utils::coeffs_to_evals;
    use crate::zpoly::zpoly;

    fn check_zpoly(line_ids: BTreeSet<usize>) {
        let coeffs = zpoly(line_ids.clone());
        dbg!(coeffs.len());
        assert_eq!(coeffs.len(), line_ids.len() * BLOB_COL_N + 1);
        let evals = coeffs_to_evals(coeffs);
        let zeros: Vec<_> = line_ids
            .iter()
            .flat_map(|idx| evals[(idx * BLOB_COL_N)..(idx + 1) * BLOB_COL_N].iter())
            .collect();
        let all_zeros = zeros.iter().all(|x| **x == Scalar::zero());
        assert!(all_zeros);
    }
    #[test]
    fn test_zpoly() {
        check_zpoly(BTreeSet::from([0]));
        check_zpoly(BTreeSet::from([BLOB_ROW_N + 1, BLOB_ROW_N * 2]));
        check_zpoly(BTreeSet::from([BLOB_ROW_N, BLOB_ROW_N + 1]));
        let mut all: BTreeSet<usize> = (0..BLOB_ROW_N).collect();
        check_zpoly(all.clone());
        all = (0..BLOB_ROW_N * 2).collect();
        check_zpoly(all.clone());
        all = (0..BLOB_ROW_ENCODED).collect();
        check_zpoly(all.clone());
        all.remove(&(BLOB_ROW_N * 2 - 1));
        check_zpoly(all);
    }
}

