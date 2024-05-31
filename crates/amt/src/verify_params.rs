use std::{fs::File, path::Path};

use crate::{amtp_verify_file_name, error, AMTParams};

use crate::ec_algebra::{Fr, G1Aff, G2Aff, Pairing, G1, G2};

use ark_bn254::Bn254;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use tracing::{debug, info, instrument};

use crate::proofs::{AmtProofError, Proof};

use ark_ec::VariableBaseMSM;

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct AMTVerifyParams<PE: Pairing> {
    pub basis: Vec<G1Aff<PE>>,
    pub vanishes: Vec<Vec<G2Aff<PE>>>,
    pub g2: G2<PE>,
    pub high_g2: G2<PE>,
}

impl AMTVerifyParams<Bn254> {
    pub fn from_dir_mont(
        dir: impl AsRef<Path>, expected_depth: usize, verify_depth: usize,
        coset: usize,
        expected_high_depth: usize,
    ) -> Self {
        Self::from_dir_inner(&dir, expected_depth, verify_depth, coset, expected_high_depth, || {
            AMTParams::<Bn254>::from_dir_mont(
                &dir,
                expected_depth,
                false,
                coset,
                expected_high_depth,
            )
        })
    }
}

impl<PE: Pairing> AMTVerifyParams<PE> {
    pub fn from_dir(
        dir: impl AsRef<Path>, expected_depth: usize, verify_depth: usize,
        coset: usize,
        expected_high_depth: usize,
    ) -> Self {
        Self::from_dir_inner(&dir, expected_depth, verify_depth, coset, expected_high_depth, || {
            AMTParams::<PE>::from_dir(&dir, expected_depth, false, coset, expected_high_depth)
        })
    }

    #[instrument(skip_all, name = "load_amt_verify_params", level = 2, parent = None, fields(depth=expected_depth, verify_depth, coset))]
    fn from_dir_inner(
        dir: impl AsRef<Path>, expected_depth: usize, verify_depth: usize,
        coset: usize, expected_high_depth: usize,
        make_prover_params: impl Fn() -> AMTParams<PE>,
    ) -> Self {
        debug!(
            depth = expected_depth,
            verify_depth, coset, "Load AMT verify params"
        );

        let file_name =
            amtp_verify_file_name::<PE>(expected_depth, verify_depth, coset, expected_high_depth);
        let path = dir.as_ref().join(file_name);

        if let Ok(params) = Self::load_cached(&path) {
            return params;
        }

        info!("Fail to load AMT verify params, recover from AMT params");

        let amt_params = make_prover_params();
        let verify_params = Self {
            basis: amt_params.basis.clone(),
            vanishes: amt_params.vanishes[0..verify_depth].to_vec(),
            g2: amt_params.g2,
            high_g2: amt_params.high_g2,
        };

        let buffer = File::create(&path).unwrap();

        info!(file = ?path, "Save recovered AMT verify params");
        verify_params.serialize_uncompressed(&buffer).unwrap();

        verify_params
    }

    fn load_cached(file: impl AsRef<Path>) -> Result<Self, error::Error> {
        let mut buffer = File::open(file)?;
        Ok(CanonicalDeserialize::deserialize_uncompressed_unchecked(
            &mut buffer,
        )?)
    }
}

impl<PE: Pairing> AMTVerifyParams<PE>
where G1<PE>: VariableBaseMSM<MulBase = G1Aff<PE>>
{
    pub fn verify_proof(
        &self, ri_data: &[Fr<PE>], batch_index: usize, proof: &Proof<PE>,
        high_commitment: G1<PE>,
        commitment: G1<PE>,
    ) -> Result<(), AmtProofError> {
        verify_amt_proof(
            &self.basis,
            &self.vanishes,
            ri_data,
            batch_index,
            proof,
            commitment,
            &self.g2,
            high_commitment,
            &self.high_g2,
        )
    }
}

pub fn verify_amt_proof<PE: Pairing>(
    basis: &[G1Aff<PE>], vanishes: &[Vec<G2Aff<PE>>], ri_data: &[Fr<PE>],
    batch_index: usize, proof: &Proof<PE>, commitment: G1<PE>, g2: &G2<PE>,
    high_commitment: G1<PE>, high_g2: &G2<PE>,
) -> Result<(), AmtProofError>
where
    G1<PE>: VariableBaseMSM<MulBase = G1Aff<PE>>,
{
    use AmtProofError::*;

    let proof_depth = proof.len();
    let num_batch = 1 << proof_depth;
    let batch = basis.len() / num_batch;

    if batch != ri_data.len() {
        return Err(UnexpectedDataLength);
    }
    if batch_index >= num_batch {
        return Err(IncorrectPosition);
    }
    assert!(batch_index < num_batch);

    let self_commitment: G1<PE> = VariableBaseMSM::msm(
        &basis[batch_index * batch..(batch_index + 1) * batch],
        ri_data,
    )
    .unwrap();

    let mut overall_commitment = self_commitment;
    for (d, (commitment, quotient)) in proof.iter().enumerate().rev() {
        let vanish_index = batch_index >> (proof_depth - 1 - d);
        let vanish = vanishes[d][vanish_index ^ 1];
        if PE::pairing(commitment, g2) != PE::pairing(quotient, vanish) {
            return Err(KzgError(d));
        }
        overall_commitment += commitment;
    }
    if overall_commitment != commitment {
        return Err(InconsistentCommitment);
    }
    if PE::pairing(commitment, high_g2) != PE::pairing(high_commitment, g2) {
        Err(FailedLowDegreeTest)
    } 
    else {
        Ok(())
    }
}
