use ark_ec::{pairing::PairingOutput, CurveGroup, VariableBaseMSM};
use std::sync::Arc;

use ark_ff::Field;
use ark_std::{cfg_into_iter, cfg_iter, UniformRand};
use rand::rngs::OsRng;
use std::sync::Mutex;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    ec_algebra::{Fr, G2Aff, Pairing, G1},
    AmtProofError,
};

pub type PairingTask<PE> =
    (G1<PE>, G2Aff<PE>, G1<PE>, G2Aff<PE>, AmtProofError);

type VerifierInner<PE> = (
    Vec<G1<PE>>,
    Vec<G2Aff<PE>>,
    Vec<G1<PE>>,
    Vec<G2Aff<PE>>,
    Vec<AmtProofError>,
);
#[derive(Clone)]
pub struct DeferredVerifier<PE: Pairing>(Arc<Mutex<VerifierInner<PE>>>);

impl<PE: Pairing> Default for DeferredVerifier<PE> {
    fn default() -> Self { Self::new() }
}

impl<PE: Pairing> DeferredVerifier<PE> {
    pub fn new() -> Self { Self(Arc::new(Mutex::new(Default::default()))) }

    pub fn record_pairing(&self, tasks: Vec<PairingTask<PE>>) {
        let mut lock_guard = self.0.lock().unwrap();
        let (va, vb, vc, vd, verror) = &mut *lock_guard;
        for (a, b, c, d, error) in tasks.into_iter() {
            va.push(a);
            vb.push(b);
            vc.push(c);
            vd.push(d);
            verror.push(error);
        }
    }

    pub fn fast_check(&self) -> bool {
        let lock_guard = self.0.lock().unwrap();
        let (va, vb, vc, vd, _) = &*lock_guard;

        let n = va.len() as u64;
        if n == 0 {
            return true;
        }

        let tau = Fr::<PE>::rand(&mut OsRng);
        let coeff: Vec<Fr<PE>> =
            cfg_into_iter!(0..n).map(|pow| tau.pow([pow])).collect();

        let left = pairing_rlc::<PE>(va, vb, &coeff);
        let right = pairing_rlc::<PE>(vc, vd, &coeff);

        left == right
    }

    pub fn check(&self) -> Result<(), AmtProofError> {
        let lock_guard = self.0.lock().unwrap();
        let (va, vb, vc, vd, ve) = &*lock_guard;

        let n = va.len();
        if n == 0 {
            return Ok(());
        }

        for i in 0..n {
            if PE::pairing(va[i], vb[i]) != PE::pairing(vc[i], vd[i]) {
                return Err(ve[i]);
            }
        }

        Ok(())
    }
}

fn all_same<T: Eq>(input: &[T]) -> bool { input.iter().all(|x| *x == input[0]) }

fn pairing_rlc<PE: Pairing>(
    g1: &[G1<PE>], g2: &[G2Aff<PE>], coeff: &[Fr<PE>],
) -> PairingOutput<PE> {
    if all_same(g2) {
        let g1 = CurveGroup::normalize_batch(g1);
        let combined: G1<PE> = VariableBaseMSM::msm(&g1[..], coeff).unwrap();
        PE::pairing(combined, g2[0])
    } else {
        let g1: Vec<_> = cfg_iter!(g1)
            .zip(cfg_iter!(coeff))
            .map(|(x, y)| *x * *y)
            .collect();
        let g1 = CurveGroup::normalize_batch(&g1[..]);
        PE::multi_pairing(g1, g2)
    }
}
