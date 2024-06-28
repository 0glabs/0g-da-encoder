use super::AMTParams;
use crate::proofs::AllProofs;
use ag_cuda_ec::pairing_suite::PE;
use ark_std::cfg_iter;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::ec_algebra::{AffineRepr, CurveGroup, Fr, G1Aff, G1};
use ag_types::PrimeFieldRepr;
use parking_lot::{
    RwLockReadGuard as ReadGuard, RwLockUpgradableReadGuard as LockGuard,
    RwLockWriteGuard as WriteGuard,
};

use ag_cuda_ec::multiexp::{multiple_multiexp_mt, upload_multiexp_bases_st};
pub const WINDOW_SIZE: usize = 8;
pub const NEG_IS_CHEAP: bool = true;

fn aggregate_line(line: &[G1<PE>], chunk_size: usize) -> Vec<G1Aff<PE>> {
    let aggregated = if chunk_size == 1 {
        line.to_vec()
    } else {
        line.chunks_exact(chunk_size)
            .map(|x| x.iter().sum::<G1<PE>>())
            .collect()
    };
    CurveGroup::normalize_batch(&aggregated)
}

impl AMTParams<PE> {
    pub(crate) fn read_gpu_bases(
        &self,
    ) -> ReadGuard<'_, Option<MsmBasisOnDevice>> {
        let device_mem = self.device_mem.upgradable_read();

        if let Some(MsmBasisOnDevice(_)) = &*device_mem {
            return LockGuard::downgrade(device_mem);
        }

        let mut device_mem = LockGuard::upgrade(device_mem);

        let quotients = self.quotients.iter().flatten();
        let basis = self.basis.iter();
        let high_basis = self.high_basis.iter();

        let to_upload: Vec<_> =
            quotients.chain(basis).chain(high_basis).copied().collect();
        let device_data = upload_multiexp_bases_st(&to_upload[..]).unwrap(); // TODO: multiple calls may
                                                                             // fail: ContextAlreadyInUse
        *device_mem = Some(MsmBasisOnDevice(device_data));

        WriteGuard::downgrade(device_mem)
    }

    pub fn gen_all_proofs_gpu(
        &self, ri_data: &[Fr<PE>],
    ) -> (G1<PE>, AllProofs<PE>) {
        let input_len: usize = self.len();
        assert_eq!(ri_data.len(), input_len);

        let height = self.quotients.len();
        let num_batches = 1usize << height;

        let guard = self.read_gpu_bases();
        let gpu_bases = guard
            .as_ref()
            .map(|MsmBasisOnDevice(pointer)| pointer)
            .unwrap();

        let exponents: Vec<_> = cfg_iter!(ri_data, 1024)
            .map(PrimeFieldRepr::to_bigint)
            .collect();

        assert_eq!(gpu_bases.size(), input_len * (height + 2));
        assert_eq!(exponents.len(), input_len);

        let lines: Vec<_> = multiple_multiexp_mt(
            gpu_bases,
            &exponents,
            num_batches,
            WINDOW_SIZE,
            NEG_IS_CHEAP,
        )
        .unwrap();

        self.process_gpu_output(lines, height)
    }

    fn process_gpu_output(
        &self, lines: Vec<G1<PE>>, height: usize,
    ) -> (G1<PE>, AllProofs<PE>) {
        let num_batches = 1usize << height;
        let batch_size = self.len() / num_batches;

        assert_eq!(lines.len(), (height + 2) * num_batches);

        let (raw_proofs, last_layers) = lines.split_at(height * num_batches);
        let (last_layer_comm, last_layer_ldt) =
            last_layers.split_at(num_batches);

        let (commitment, commitments) =
            self.build_commitment_tree(last_layer_comm);

        let proofs: Vec<Vec<G1Aff<PE>>> = raw_proofs
            .chunks_exact(num_batches)
            .enumerate()
            .map(|(d, line)| aggregate_line(line, num_batches >> (d + 1)))
            .collect();

        assert_eq!(height, proofs.len());

        let high_commitment =
            last_layer_ldt.iter().sum::<G1<PE>>().into_affine();

        let all_proofs = AllProofs {
            commitments,
            proofs,
            input_len: self.len(),
            batch_size,
            high_commitment,
        };
        (commitment.into_group(), all_proofs)
    }
}

pub(crate) struct MsmBasisOnDevice(
    ag_cuda_ec::multiexp::DeviceAffineSlice<G1Aff<PE>>,
);

// A raw GPU pointer is inside
// `MsmBasisOnDevice`, since we only
// read this pointer, it can be declared
// as `Send + Sync`. However, the
// visibility of inside pointer should
// be restricted in this module to avoid
// misuse.
unsafe impl Send for MsmBasisOnDevice {}
unsafe impl Sync for MsmBasisOnDevice {}

#[cfg(all(test, feature = "cuda-bn254"))]
mod tests {
    use super::super::tests::{random_scalars, AMT, TEST_LENGTH, TEST_LEVEL};

    #[test]
    fn test_proof_verify_gpu() {
        let ri_data = &random_scalars(TEST_LENGTH);
        let commitment = AMT.commitment(ri_data);

        for log_batch in 0..TEST_LEVEL {
            let prove_depth = TEST_LEVEL - log_batch;
            let batch = 1 << log_batch;
            let all_proofs = AMT
                .reduce_prove_depth(prove_depth)
                .gen_all_proofs_gpu(ri_data)
                .1;
            for (index, data) in ri_data.chunks_exact(batch).enumerate() {
                let (proof, high_commitment) = all_proofs.get_proof(index);
                AMT.verify_proof(
                    &data,
                    index,
                    &proof,
                    high_commitment.into(),
                    commitment,
                )
                .unwrap();
            }
        }
    }
}
