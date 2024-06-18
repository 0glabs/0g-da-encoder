use std::collections::BTreeMap;

use zg_encoder::constants::Scalar;
use ark_ff::{Zero, One};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::log2;
use crate::utils::{ifft_allow_all_zeros, many_non_zeros};

#[derive(Clone, Debug)]
pub enum Poly {
    Dense(Vec<Scalar>), // all coeffs, degree = vec.len() - 1
    Sparse(BTreeMap<usize, Scalar>), // key: j, value: f_j; f_{others} = 0
    One(()) // f = 1
}

impl Poly {
    fn len(&self) -> usize {
        match self {
            Poly::One(_) => 1,
            Poly::Sparse(inner) => inner.len(),
            Poly::Dense(inner) => inner.len()
        }
    }
    pub fn degree(&self) -> usize {
        match self {
            Poly::One(_) => 0,
            Poly::Sparse(inner) => *inner.last_key_value().unwrap().0,
            Poly::Dense(inner) => inner.len() - 1
        }
    }
}

impl Poly {
    pub fn poly_all_zeros(degree: usize) -> Poly {
        assert!(degree.is_power_of_two());
        let mut res = BTreeMap::new();
            res.insert(degree, Scalar::one());
            res.insert(0, -Scalar::one());
            Poly::Sparse(res)
    }
}

impl Poly {
    pub fn multiply(&self, other: &Poly) -> Poly {
        if let Poly::One(_) = self {
            return other.clone()
        }
        if let Poly::One(_) = other {
            return self.clone()
        }
        let sparse_complexity = self.len() * other.len();
        let res_degree = self.degree() + other.degree();
        let fft_degree = (res_degree + 1).next_power_of_two();
        let dense_complexity = 3 * fft_degree * log2(fft_degree) as usize;
        if sparse_complexity < dense_complexity {
            self.multiply_sparse(other, res_degree)
        }
        else {
            self.multiply_dense(other, res_degree)
        }
    }
    
    fn multiply_sparse(&self, other: &Poly, res_degree: usize) -> Poly {
        match (self, other) {
            (Poly::Dense(dense_1), Poly::Dense(dense_2)) => {
                let mut res = vec![Scalar::zero(); res_degree + 1];
                for (dense_idx_1, dense_coeff_1) in dense_1.iter().enumerate() {
                    for (dense_idx_2, dense_coeff_2) in dense_2.iter().enumerate() {
                        res[dense_idx_1 + dense_idx_2] += dense_coeff_1 * dense_coeff_2;
                    }
                }
                assert_ne!(res[res_degree], Scalar::zero());
                Poly::from_vec_uncheck(res)
            },

            (Poly::Dense(dense), Poly::Sparse(sparse)) | (Poly::Sparse(sparse), Poly::Dense(dense)) => {
                let mut res = vec![Scalar::zero(); res_degree + 1];
                for (dense_idx, dense_coeff) in dense.iter().enumerate() {
                    for (sparse_idx, sparse_coeff) in sparse {
                        res[dense_idx + sparse_idx] += dense_coeff * sparse_coeff;
                    }
                }
                assert_ne!(res[res_degree], Scalar::zero());
                Poly::from_vec_uncheck(res)
            },

            (Poly::Sparse(sparse_1), Poly::Sparse(sparse_2)) => {
                let mut res = BTreeMap::new();
                for (idx_1, coeff_1) in sparse_1 {
                    for (idx_2, coeff_2) in sparse_2 {
                        let idx = idx_1 + idx_2;
                        let coeff = coeff_1 * coeff_2;
                        res
                            .entry(idx)
                            .and_modify(|curr_coeff| *curr_coeff += coeff)
                            .or_insert(coeff);
                    }
                }
                assert_ne!(*res.last_key_value().unwrap().1, Scalar::zero());
                let keys_to_remove: Vec<usize> = res.iter()
                    .filter(|&(_, value)| *value == Scalar::zero())
                    .map(|(&key, _)| key)
                    .collect();
                for key in keys_to_remove {
                    res.remove(&key);
                }
                Poly::Sparse(res)
            },

            _ => panic!("Poly::One should not invoke multiply_sparse")
        }
    }


    fn multiply_dense(&self, other: &Poly, res_degree: usize) -> Poly {
        let coeffs_1 = self.to_vec_extend(res_degree + 1);
        let coeffs_2 = other.to_vec_extend(res_degree + 1);
        let fft_degree = (res_degree + 1).next_power_of_two();
        let fft_domain = Radix2EvaluationDomain::<Scalar>::new(fft_degree).unwrap();
        let evals_1 = fft_domain.fft(&coeffs_1);
        let evals_2 = fft_domain.fft(&coeffs_2);
        let evals: Vec<Scalar> = evals_1.iter().zip(evals_2.iter())
            .map(|(x, y)| x * y)
            .collect();
        // #(evals == 0) <= res_degree
        // fft_degree >= res_degree + 1 > res_degree
        // thus, evals contain at least one non-zero element
        // therefore, directly ifft is Ok
        let coeffs = fft_domain.ifft(&evals);
        let all_zeros = coeffs[(res_degree + 1)..].iter().all(|&x| x == Scalar::zero());
        assert!(all_zeros);
        assert_ne!(coeffs[res_degree], Scalar::zero());
        Poly::from_vec_uncheck(coeffs[..(res_degree + 1)].to_vec())
    }
}

impl Poly {
    pub fn to_vec(&self) -> Vec<Scalar> {
        let res = match self {
            Poly::Dense(dense) => {
                dense.to_vec()
            },
            Poly::Sparse(sparse) => {
                let mut res = vec![Scalar::zero(); self.degree() + 1];
                for (idx, coeff) in sparse {
                    res[*idx] = *coeff;
                }
                res
            }
            _ => vec![Scalar::one()]
        };
        assert_ne!(res[res.len() - 1], Scalar::zero());
        res
    }
    pub fn to_vec_extend(&self, new_size: usize) -> Vec<Scalar> {
        match self {
            Poly::Dense(dense) => {
                let extend_dense: Vec<Scalar> = vec![Scalar::zero(); new_size - dense.len()];
                let dense_origin: Vec<Scalar> = dense.clone();
                [dense_origin, extend_dense].concat()
            },
            Poly::Sparse(sparse) => {
                let mut res = vec![Scalar::zero(); new_size];
                for (idx, coeff) in sparse {
                    res[*idx] = *coeff;
                }
                res
            }
            _ => panic!("Poly::One should not invoke to_vec_extend")
        }
    }

    // vec[-1] != Scalar::zero() has been ensured
    pub fn from_vec_uncheck(vec: Vec<Scalar>) -> Self {
        if many_non_zeros(&vec) {
            Poly::Dense(vec)
        }
        else {
            Poly::sparse_from_vec(vec)
        }
    }
    pub fn dense_from_vec(vec: &[Scalar]) -> Self {
        let mut res = vec.to_vec();
        if let Some(last_pos) = res.iter().rposition(|&value| value != Scalar::zero()) {
            res.truncate(last_pos + 1);
        } else {
            res.clear();
        }
        Poly::Dense(res)
    }
    pub fn sparse_from_vec(vec: Vec<Scalar>) -> Self {
        let res: BTreeMap<usize, Scalar> = vec.into_iter().enumerate()
            .filter(|&(_, ref value)| *value != Scalar::zero())
            .map(|(idx, element)| (idx, element))
            .collect();
        Poly::Sparse(res)
    }
}

impl PartialEq for Poly {
    fn eq(&self, other: &Self) -> bool {
        let degree = self.degree();
        if degree != other.degree() {
            return false
        }
        match (self, other) {
            (Self::One(_), Self::Dense(inner)) 
            | (Self::Dense(inner), Self::One(_)) 
                => inner[0] == Scalar::one(),
            (Self::One(_), Self::Sparse(inner)) 
            | (Self::Sparse(inner), Self::One(_)) 
                => {if let Some(value) = inner.get(&0) {
                        *value == Scalar::one()
                    } else {
                        false
                    }},
            _ => {
                let self_inner = self.to_vec_extend(degree + 1);
                let other_inner = self.to_vec_extend(degree + 1);
                self_inner == other_inner
            }
        }
    }
}

mod tests {
    use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
    use ark_std::rand;
    use zg_encoder::constants::Scalar;
    use ark_ff::{One, Zero, Field};
    use rand::seq::SliceRandom;
    use rand::thread_rng;  
    use crate::utils::random_scalars;
    
    use super::Poly;

    #[test]
    fn test_dense_sparse_consistency() {
        let length = 1 << 4;
        let vec = [vec![Scalar::zero(); length], random_scalars(length)].concat();
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
        let length = 1 << 4;
        let mut vec = [vec![Scalar::zero(); length], random_scalars(length)].concat();
        let mut rng = thread_rng();
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

    #[test]
    fn test_poly_all_zeros() {
        for log_degree in 0..14 {
            let degree = 1 << log_degree;
            let poly = Poly::poly_all_zeros(degree);
            let coeffs = poly.to_vec_extend(degree + 1);
            assert_ne!(coeffs[degree], Scalar::zero());
            let fft_domain = Radix2EvaluationDomain::<Scalar>::new(degree * 2).unwrap();
            let evals = fft_domain.fft(&coeffs);
            let zeros: Vec<Scalar> = evals.iter().step_by(2).cloned().collect();
            let non_zeros: Vec<Scalar> = evals.iter().skip(1).step_by(2).cloned().collect();
            assert!(zeros.iter().all(|&x| x == Scalar::zero()));
            assert!(non_zeros.iter().all(|&x| x == -(Scalar::one()).double()));
        }
    }
}