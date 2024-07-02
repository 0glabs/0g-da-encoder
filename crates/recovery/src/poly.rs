#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::collections::BTreeMap;

use crate::utils::many_non_zeros;
use ark_ff::{One, Zero};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::{cfg_iter, log2};
use zg_encoder::{cfg_chunks_exact, constants::Scalar};

#[derive(Clone, Debug)]
pub enum Poly {
    Dense(Vec<Scalar>), // all coeffs, degree = vec.len() - 1
    Sparse(BTreeMap<usize, Scalar>), // key: j, value: f_j; f_{others} = 0
    One(()),            // f = 1
}

impl Poly {
    fn len(&self) -> usize {
        match self {
            Poly::One(_) => 0,
            Poly::Sparse(inner) => inner.len(),
            Poly::Dense(inner) => inner.len(),
        }
    }

    pub fn degree(&self) -> usize {
        match self {
            Poly::One(_) => 0,
            Poly::Sparse(inner) => inner.last_key_value().map_or(0, |x| *x.0),
            Poly::Dense(inner) => inner.len().saturating_sub(1),
        }
    }

    pub fn is_one(&self) -> bool {
        if self.degree() != 0 {
            return false;
        }

        match self {
            Poly::Dense(inner) => {
                inner.first().map_or(false, |v| *v == Scalar::one())
            }
            Poly::Sparse(inner) => {
                inner.get(&0).map_or(false, |v| *v == Scalar::one())
            }
            Poly::One(_) => true,
        }
    }
}

impl Poly {
    pub fn multiply(&self, other: &Poly) -> Poly {
        if let Poly::One(_) = self {
            return other.clone();
        }
        if let Poly::One(_) = other {
            return self.clone();
        }
        let sparse_complexity = self.len() * other.len();
        let res_degree = self.degree() + other.degree();
        let fft_degree = (res_degree + 1).next_power_of_two();
        let dense_complexity = 3 * fft_degree * log2(fft_degree) as usize;
        if sparse_complexity < dense_complexity {
            self.multiply_sparse(other, res_degree)
        } else {
            self.multiply_dense(other, res_degree)
        }
    }

    fn multiply_sparse(&self, other: &Poly, res_degree: usize) -> Poly {
        match (self, other) {
            (Poly::Dense(dense_1), Poly::Dense(dense_2)) => {
                Self::mul_dense_by_dense(dense_1, dense_2, res_degree)
            }

            (Poly::Dense(dense), Poly::Sparse(sparse))
            | (Poly::Sparse(sparse), Poly::Dense(dense)) => {
                Self::mul_dense_by_sparse(dense, sparse, res_degree)
            }

            (Poly::Sparse(sparse_1), Poly::Sparse(sparse_2)) => {
                Self::mul_sparse_by_sparse(sparse_1, sparse_2)
            }

            _ => unreachable!("Poly::One should not invoke multiply_sparse"),
        }
    }

    fn mul_dense_by_dense(
        dense_1: &[Scalar], dense_2: &[Scalar], res_degree: usize,
    ) -> Poly {
        let mut res = vec![Scalar::zero(); res_degree + 1];
        for (dense_idx_1, dense_coeff_1) in dense_1.iter().enumerate() {
            for (dense_idx_2, dense_coeff_2) in dense_2.iter().enumerate() {
                res[dense_idx_1 + dense_idx_2] += dense_coeff_1 * dense_coeff_2;
            }
        }
        assert_ne!(res[res_degree], Scalar::zero());
        Poly::from_vec(res)
    }

    fn mul_dense_by_sparse(
        dense: &[Scalar], sparse: &BTreeMap<usize, Scalar>, res_degree: usize,
    ) -> Poly {
        let mut res = vec![Scalar::zero(); res_degree + 1];
        for (dense_idx, dense_coeff) in dense.iter().enumerate() {
            for (sparse_idx, sparse_coeff) in sparse {
                res[dense_idx + sparse_idx] += dense_coeff * sparse_coeff;
            }
        }
        Poly::from_vec(res)
    }

    fn mul_sparse_by_sparse(
        sparse_1: &BTreeMap<usize, Scalar>, sparse_2: &BTreeMap<usize, Scalar>,
    ) -> Poly {
        let mut res = BTreeMap::new();
        for (idx_1, coeff_1) in sparse_1 {
            for (idx_2, coeff_2) in sparse_2 {
                let idx = idx_1 + idx_2;
                let coeff = coeff_1 * coeff_2;
                res.entry(idx)
                    .and_modify(|curr_coeff| *curr_coeff += coeff)
                    .or_insert(coeff);
            }
        }
        if !res.is_empty() {
            assert_ne!(*res.last_key_value().unwrap().1, Scalar::zero());
        }
        let keys_to_remove: Vec<usize> = res
            .iter()
            .filter(|&(_, value)| *value == Scalar::zero())
            .map(|(&key, _)| key)
            .collect();
        for key in keys_to_remove {
            res.remove(&key);
        }
        Poly::Sparse(res)
    }

    fn multiply_dense(&self, other: &Poly, res_degree: usize) -> Poly {
        let coeffs_1 = self.to_vec_extend(res_degree + 1);
        let coeffs_2 = other.to_vec_extend(res_degree + 1);
        let fft_degree = (res_degree + 1).next_power_of_two();
        let fft_domain =
            Radix2EvaluationDomain::<Scalar>::new(fft_degree).unwrap();
        let evals_1 = fft_domain.fft(&coeffs_1);
        let evals_2 = fft_domain.fft(&coeffs_2);
        let evals: Vec<Scalar> = cfg_iter!(evals_1)
            .zip(cfg_iter!(evals_2))
            .map(|(x, y)| x * y)
            .collect();
        // #(evals == 0) <= res_degree
        // fft_degree >= res_degree + 1 > res_degree
        // thus, evals contain at least one non-zero element
        // therefore, directly ifft is Ok
        let coeffs = fft_domain.ifft(&evals);
        let all_zeros =
            cfg_iter!(coeffs[(res_degree + 1)..]).all(|&x| x == Scalar::zero());
        assert!(all_zeros);
        assert_ne!(coeffs[res_degree], Scalar::zero());
        Poly::from_vec(coeffs[..(res_degree + 1)].to_vec())
    }
}

pub fn polys_multiply(polys_ori: &[Poly]) -> Poly {
    let num = polys_ori.len();
    let num_power_2 = num.next_power_of_two();
    let mut polys = {
        if num < num_power_2 {
            [polys_ori.to_vec(), vec![Poly::One(()); num_power_2 - num]]
                .concat()
        } else {
            polys_ori.to_vec()
        }
    };
    let num_iter = log2(num_power_2) as usize;
    for _ in 0..num_iter {
        polys = cfg_chunks_exact!(polys, 2)
            .map(|x| x[0].multiply(&x[1]))
            .collect::<Vec<_>>();
    }
    assert_eq!(polys.len(), 1);
    polys.pop().unwrap()
}

impl Poly {
    pub fn to_vec(&self) -> Vec<Scalar> {
        let mut all_zeros = false;
        let res = match self {
            Poly::Dense(dense) => {
                if dense.is_empty() {
                    all_zeros = true;
                    vec![Scalar::zero()]
                } else {
                    dense.to_vec()
                }
            }
            Poly::Sparse(sparse) => {
                if sparse.is_empty() {
                    all_zeros = true;
                }
                let mut res = vec![Scalar::zero(); self.degree() + 1];
                for (idx, coeff) in sparse {
                    res[*idx] = *coeff;
                }
                res
            }
            _ => vec![Scalar::one()],
        };
        if !all_zeros {
            assert_ne!(res[res.len() - 1], Scalar::zero());
        }
        res
    }

    pub fn to_vec_extend(&self, new_size: usize) -> Vec<Scalar> {
        assert!(new_size > self.degree());
        match self {
            Poly::Dense(dense) => {
                let extend_dense: Vec<Scalar> =
                    vec![Scalar::zero(); new_size - dense.len()];
                [dense.to_vec(), extend_dense].concat()
            }
            Poly::Sparse(sparse) => {
                let mut res = vec![Scalar::zero(); new_size];
                for (idx, coeff) in sparse {
                    res[*idx] = *coeff;
                }
                res
            }
            _ => unreachable!("Poly::One should not invoke to_vec_extend"),
        }
    }

    pub fn from_vec(vec: Vec<Scalar>) -> Self {
        if many_non_zeros(&vec) {
            Poly::dense_from_vec(&vec)
        } else {
            Poly::sparse_from_vec(vec)
        }
    }

    pub fn dense_from_vec(vec: &[Scalar]) -> Self {
        let mut res = vec.to_vec();
        if let Some(last_pos) =
            res.iter().rposition(|&value| value != Scalar::zero())
        {
            res.truncate(last_pos + 1);
        } else {
            res.clear();
        }
        Poly::Dense(res)
    }

    pub fn sparse_from_vec(vec: Vec<Scalar>) -> Self {
        let res: BTreeMap<usize, Scalar> = vec
            .into_iter()
            .enumerate()
            .filter(|(_, value)| *value != Scalar::zero())
            .collect();
        Poly::Sparse(res)
    }
}

impl PartialEq for Poly {
    fn eq(&self, other: &Self) -> bool {
        let degree = self.degree();
        if degree != other.degree() {
            return false;
        }
        match (self, other) {
            (Self::One(_), another) | (another, Self::One(_)) => {
                another.is_one()
            }
            _ => {
                let self_inner = self.to_vec_extend(degree + 1);
                let other_inner = self.to_vec_extend(degree + 1);
                self_inner == other_inner
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::random_scalars;
    use ark_ff::Zero;
    use ark_std::rand;
    use rand::{seq::SliceRandom, thread_rng};
    use zg_encoder::constants::Scalar;

    use super::Poly;

    #[test]
    fn test_poly_from_vec_all_zeros() {
        let vec = vec![Scalar::zero(); 16];
        let dense_poly = Poly::dense_from_vec(&vec);
        assert_eq!(dense_poly.degree(), 0);
        assert_eq!(dense_poly.len(), 0);
        assert_eq!(dense_poly, Poly::sparse_from_vec(vec));
        assert_ne!(dense_poly, Poly::One(()));
    }

    #[test]
    fn test_dense_sparse_consistency() {
        let mut rng = thread_rng();
        let length = 1 << 4;
        let vec = [
            vec![Scalar::zero(); length],
            random_scalars(length, &mut rng),
        ]
        .concat();
        let dense = Poly::dense_from_vec(&vec);
        let sparse = Poly::sparse_from_vec(vec);
        assert_eq!(dense, sparse);
    }

    fn all_elements_same<T: PartialEq>(vec: &[T]) -> bool {
        if let Some(first) = vec.first() {
            vec.iter().all(|item| item == first)
        } else {
            true
        }
    }

    #[test]
    fn test_multiply_consistency() {
        let mut rng = thread_rng();
        let length = 1 << 4;
        let mut vec = [
            vec![Scalar::zero(); length],
            random_scalars(length, &mut rng),
        ]
        .concat();
        vec.shuffle(&mut rng);
        let dense = Poly::dense_from_vec(&vec);
        let sparse = Poly::sparse_from_vec(vec);
        let mut mul_res = vec![];
        let res_degree = dense.degree() + sparse.degree();
        mul_res.push(dense.multiply_sparse(&sparse, res_degree));
        mul_res.push(sparse.multiply_sparse(&sparse, res_degree));
        mul_res.push(sparse.multiply_sparse(&dense, res_degree));
        mul_res.push(sparse.multiply_dense(&sparse, res_degree));
        mul_res.push(sparse.multiply_dense(&dense, res_degree));
        mul_res.push(dense.multiply_dense(&sparse, res_degree));
        mul_res.push(dense.multiply_dense(&dense, res_degree));
        mul_res.push(sparse.multiply(&dense));
        mul_res.push(dense.multiply(&sparse));
        mul_res.push(sparse.multiply(&sparse));
        mul_res.push(dense.multiply(&dense));
        mul_res.push(mul_res[0].multiply(&Poly::One(())));
        mul_res.push(Poly::One(()).multiply(&mul_res[0]));
        assert!(all_elements_same(&mul_res));
    }
}
