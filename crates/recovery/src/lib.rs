mod data_poly;
mod data_times_zpoly;
mod error;
mod poly;
mod utils;
mod zpoly;

#[test]
fn test_fft_all_zeros() {
    use ark_ff::Zero;
    use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
    use zg_encoder::constants::Scalar;
    let size = 16usize;
    let fft_domain = Radix2EvaluationDomain::<Scalar>::new(size * 2).unwrap();
    let evals: Vec<Scalar> = vec![Scalar::zero(); size];
    let coeffs = fft_domain.ifft(&evals);
    assert!(coeffs.iter().all(|&x| x == Scalar::zero()));
}

#[test]
fn test_fft_has_zeros() {
    use ark_ff::{One, Zero};
    use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
    use zg_encoder::constants::Scalar;
    let size = 16usize;
    let fft_domain = Radix2EvaluationDomain::<Scalar>::new(size).unwrap();
    let mut evals: Vec<Scalar> = vec![Scalar::zero(); size];
    evals[0] = Scalar::one();
    let coeffs = fft_domain.ifft(&evals);
    assert!(!coeffs.iter().all(|&x| x == Scalar::zero()));
}
