mod poly;
mod zpoly;
mod utils;
//mod data_times_zpoly;

#[test]
fn test_fft_all_zeros() {
    use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
    use zg_encoder::constants::Scalar;
    use ark_ff::Zero;
    let size = 16usize;
    let fft_domain = Radix2EvaluationDomain::<Scalar>::new(size * 2).unwrap();
    let evals: Vec<Scalar> = vec![Scalar::zero(); size];
    let coeffs = fft_domain.ifft(&evals);
    println!("{:?}", coeffs);
}

#[test]
fn test_fft_has_zeros() {
    use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
    use zg_encoder::constants::Scalar;
    use ark_ff::{Zero, One};
    let size = 16usize;
    let fft_domain = Radix2EvaluationDomain::<Scalar>::new(size * 2).unwrap();
    let mut evals: Vec<Scalar> = vec![Scalar::zero(); size];
    evals[0] = Scalar::one();
    let coeffs = fft_domain.ifft(&evals);
    println!("{:?}", coeffs);
}