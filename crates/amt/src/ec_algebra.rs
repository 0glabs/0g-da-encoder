// Re-export all the required components
// in Arkworks's repo (original Zexe).

// Since Zexe's repo doesn't have a
// stable implementation and could be
// refactored in the future,
// we import all the required objects in
// one place and all its usage for this
// repo should import from here.

pub use ark_ec::{
    pairing::Pairing as ArkPairing, AffineRepr, CurveGroup, Group,
    VariableBaseMSM,
};
pub use ark_ff::{
    utils::k_adicity, BigInt, BigInteger, FftField, Field, One, PrimeField,
    UniformRand, Zero,
};
pub use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
pub use ark_serialize::{
    CanonicalDeserialize, CanonicalSerialize, Read, SerializationError, Write,
};

pub type G1<PE> = <PE as ark_ec::pairing::Pairing>::G1;
pub type G1Aff<PE> = <PE as ark_ec::pairing::Pairing>::G1Affine;
pub type G2<PE> = <PE as ark_ec::pairing::Pairing>::G2;
pub type G2Aff<PE> = <PE as ark_ec::pairing::Pairing>::G2Affine;
pub type Fr<PE> = <PE as ark_ec::pairing::Pairing>::ScalarField;
pub type Fq<PE> = <PE as ark_ec::pairing::Pairing>::BaseField;
pub type FrInt<PE> = <Fr<PE> as PrimeField>::BigInt;
pub type FqInt<PE> = <Fq<PE> as PrimeField>::BigInt;
pub type Fq2<PE> = <G2Aff<PE> as AffineRepr>::BaseField;

pub trait Pairing: ark_ec::pairing::Pairing {
    fn fast_fft(
        fft_domain: &Radix2EvaluationDomain<Fr<Self>>, input: &[G1<Self>],
    ) -> Vec<G1<Self>> {
        fft_domain.fft(input)
    }

    fn fast_ifft(
        fft_domain: &Radix2EvaluationDomain<Fr<Self>>, input: &[G1<Self>],
    ) -> Vec<G1<Self>> {
        fft_domain.ifft(input)
    }
}

#[cfg(not(feature = "cuda"))]
impl<PE: ark_ec::pairing::Pairing> Pairing for PE {}

#[cfg(feature = "cuda")]
mod cuda_accelerate {
    use super::{Fr, Pairing, G1};
    use ag_cuda_ec::{
        fft::{radix_fft_mt, radix_ifft_mt},
        pairing_suite::PE,
    };
    use ark_ff::Field;
    use ark_std::Zero;

    fn make_omegas(group_gen: Fr<PE>) -> Vec<Fr<PE>> {
        let mut omegas = vec![Fr::<PE>::zero(); 32];
        omegas[0] = group_gen;
        for i in 1..32 {
            omegas[i] = omegas[i - 1].square();
        }
        omegas
    }

    impl Pairing for PE {
        fn fast_fft(
            fft_domain: &ark_poly::Radix2EvaluationDomain<Fr<Self>>,
            input: &[G1<Self>],
        ) -> Vec<G1<Self>> {
            let mut answer = input.to_vec();
            radix_fft_mt(&mut answer, &make_omegas(fft_domain.group_gen), None)
                .unwrap();
            answer
        }

        fn fast_ifft(
            fft_domain: &ark_poly::Radix2EvaluationDomain<Fr<Self>>,
            input: &[G1<Self>],
        ) -> Vec<G1<Self>> {
            let mut answer = input.to_vec();
            radix_ifft_mt(
                &mut answer,
                &make_omegas(fft_domain.group_gen_inv),
                None,
                fft_domain.size_inv,
            )
            .unwrap();
            answer
        }
    }
}
