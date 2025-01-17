use std::{fmt::Debug, marker::PhantomData};

use rand::{CryptoRng, Rng, RngCore, SeedableRng};

use sha3::{digest::FixedOutputReset, Digest};

use crate::{
    circuit::Circuit,
    commitment::Commitment,
    config::HASH_LEN,
    data_structures::{FirstMessageA, PartyExecution, Proof, PublicInput},
    error::Error,
    fs::SigmaFS,
    gf2_word::{GF2Word, Value},
    key::Key,
    num_of_repetitions_given_desired_security,
    party::Party,
    tape::Tape,
};

pub struct Verifier<T: Value, TapeR, D>(PhantomData<(T, TapeR, D)>)
where
    D: Digest + FixedOutputReset,
    TapeR: SeedableRng<Seed = Key> + RngCore + CryptoRng;

impl<T, TapeR, D> Verifier<T, TapeR, D>
where
    T: Value + PartialEq,
    TapeR: SeedableRng<Seed = Key> + RngCore + CryptoRng,
    D: Clone + Default + Digest + FixedOutputReset,
{
    pub fn verify<const SIGMA: usize>(
        proof: &Proof<T, D, SIGMA>,
        circuit: &impl Circuit<T>,
        public_output: &Vec<GF2Word<T>>,
    ) -> Result<(), Error> {
        let num_of_repetitions = num_of_repetitions_given_desired_security(SIGMA);

        // Based on O3 and O5 of (https://eprint.iacr.org/2017/279.pdf)
        assert_eq!(proof.party_inputs.len(), num_of_repetitions);
        assert_eq!(proof.commitments.len(), num_of_repetitions);
        assert_eq!(proof.views.len(), num_of_repetitions);
        assert_eq!(proof.claimed_trits.len(), num_of_repetitions);
        assert_eq!(proof.keys.len(), 2 * num_of_repetitions);

        let mut all_commitments = Vec::<Commitment<D>>::with_capacity(3 * num_of_repetitions);
        let mut outputs = Vec::<Vec<GF2Word<T>>>::with_capacity(3 * num_of_repetitions);

        for (repetition, &party_index) in proof.claimed_trits.iter().enumerate() {
            let k_i0 = proof.keys[2 * repetition];
            let mut p = Party::new::<TapeR>(
                proof.party_inputs[repetition].clone(),
                k_i0,
                circuit.num_of_mul_gates(),
            );

            let k_i1 = proof.keys[2 * repetition + 1];
            let view_i1 = &proof.views[repetition];

            let tape_i1 = Tape::from_key::<TapeR>(k_i1, circuit.num_of_mul_gates());
            let mut p_next = Party::from_tape_and_view(view_i1.clone(), tape_i1);

            let (o0, o1) = circuit.simulate_two_parties(&mut p, &mut p_next)?;
            let o2 = Self::derive_third_output(public_output, circuit, (&o0, &o1));

            /*
                Based on O6 of (https://eprint.iacr.org/2017/279.pdf)
                Instead of checking view consistency, full view is computed through simulation
                then security comes from binding property of H used when committing
            */
            let view_i0 = &p.view;

            let pi0_execution = PartyExecution {
                key: &k_i0,
                view: view_i0,
            };

            // Based on O4 of (https://eprint.iacr.org/2017/279.pdf)
            let cm_i0 = pi0_execution.commit::<D>()?;

            let pi1_execution = PartyExecution {
                key: &k_i1,
                view: view_i1,
            };

            // Based on O4 of (https://eprint.iacr.org/2017/279.pdf)
            let cm_i1 = pi1_execution.commit::<D>()?;

            let cm_i2 = &proof.commitments[repetition];

            match party_index {
                0 => {
                    all_commitments.push(cm_i0);
                    all_commitments.push(cm_i1);
                    all_commitments.push(cm_i2.clone());

                    outputs.push(o0);
                    outputs.push(o1);
                    outputs.push(o2);
                }
                1 => {
                    all_commitments.push(cm_i2.clone());
                    all_commitments.push(cm_i0);
                    all_commitments.push(cm_i1);

                    outputs.push(o2);
                    outputs.push(o0);
                    outputs.push(o1);
                }
                2 => {
                    all_commitments.push(cm_i1);
                    all_commitments.push(cm_i2.clone());
                    all_commitments.push(cm_i0);

                    outputs.push(o1);
                    outputs.push(o2);
                    outputs.push(o0);
                }
                _ => panic!("Not trit"),
            };
        }

        let pi = PublicInput {
            outputs: &outputs,
            public_output,
            hash_len: HASH_LEN,
            security_param: SIGMA,
        };

        // TODO: remove hardcoded seed
        let mut fs_oracle = SigmaFS::<D>::initialize(&[0u8]);
        fs_oracle.digest_public_data(&pi)?;
        fs_oracle.digest_prover_message(&all_commitments)?;

        let opening_indices = fs_oracle.sample_trits(num_of_repetitions);
        if opening_indices != proof.claimed_trits {
            return Err(Error::FiatShamirOutputsMatchingError);
        }

        Ok(())
    }

    pub fn derive_third_output(
        public_output: &[GF2Word<T>],
        circuit: &impl Circuit<T>,
        circuit_simulation_output: (&Vec<GF2Word<T>>, &Vec<GF2Word<T>>),
    ) -> Vec<GF2Word<T>> {
        let party_output_len = circuit.party_output_len();
        let (o1, o2) = circuit_simulation_output;

        // TODO: introduce error here
        assert_eq!(o1.len(), party_output_len);
        assert_eq!(o2.len(), party_output_len);

        let mut derived_output = Vec::with_capacity(party_output_len);

        for i in 0..party_output_len {
            derived_output.push(o1[i] ^ o2[i] ^ public_output[i]);
        }

        derived_output
    }
}

#[derive(Default)]
pub struct InteractiveVerifier<T: Value, TapeR, D>
where
    D: Default + Digest + FixedOutputReset + Clone,
    TapeR: SeedableRng<Seed = Key> + RngCore + CryptoRng,
{
    challenge: Vec<u8>,
    pd: PhantomData<(T, TapeR, D)>,
    all_commitments: Vec<Commitment<D>>,
    outputs: Vec<Vec<GF2Word<T>>>,
}

impl<T, TapeR, D> InteractiveVerifier<T, TapeR, D>
where
    T: Value + PartialEq,
    TapeR: SeedableRng<Seed = Key> + RngCore + CryptoRng,
    D: Clone + Default + Digest + FixedOutputReset,
{
    pub fn new() -> Self {
        InteractiveVerifier {
            challenge: Vec::new(),
            pd: PhantomData::default(),
            all_commitments: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn round2<R: RngCore + CryptoRng>(
        &mut self,
        rng: &mut R,
        r: usize,
        fm: FirstMessageA<T, D>,
    ) -> Vec<u8> {
        let challenge: Vec<u8> = (0..r).map(|_| rng.gen_range(0..3)).collect();
        self.challenge = challenge.clone();
        self.all_commitments = fm.all_commitments;
        self.outputs = fm.outputs;

        challenge
    }

    pub fn verify<const SIGMA: usize>(
        &self,
        proof: &Proof<T, D, SIGMA>,
        circuit: &impl Circuit<T>,
        public_output: &Vec<GF2Word<T>>,
    ) -> Result<(), Error> {
        let num_of_repetitions = num_of_repetitions_given_desired_security(SIGMA);

        // Based on O3 and O5 of (https://eprint.iacr.org/2017/279.pdf)
        assert_eq!(proof.party_inputs.len(), num_of_repetitions);
        assert_eq!(proof.commitments.len(), num_of_repetitions);
        assert_eq!(proof.views.len(), num_of_repetitions);
        assert_eq!(proof.claimed_trits.len(), num_of_repetitions);
        assert_eq!(proof.keys.len(), 2 * num_of_repetitions);

        let mut all_commitments = Vec::<Commitment<D>>::with_capacity(3 * num_of_repetitions);
        let mut outputs = Vec::<Vec<GF2Word<T>>>::with_capacity(3 * num_of_repetitions);

        for (repetition, &party_index) in proof.claimed_trits.iter().enumerate() {
            let k_i0 = proof.keys[2 * repetition];
            let mut p = Party::new::<TapeR>(
                proof.party_inputs[repetition].clone(),
                k_i0,
                circuit.num_of_mul_gates(),
            );

            let k_i1 = proof.keys[2 * repetition + 1];
            let view_i1 = &proof.views[repetition];

            let tape_i1 = Tape::from_key::<TapeR>(k_i1, circuit.num_of_mul_gates());
            let mut p_next = Party::from_tape_and_view(view_i1.clone(), tape_i1);

            let (o0, o1) = circuit.simulate_two_parties(&mut p, &mut p_next)?;
            let o2 = Self::derive_third_output(public_output, circuit, (&o0, &o1));

            /*
                Based on O6 of (https://eprint.iacr.org/2017/279.pdf)
                Instead of checking view consistency, full view is computed through simulation
                then security comes from binding property of H used when committing
            */
            let view_i0 = &p.view;

            let pi0_execution = PartyExecution {
                key: &k_i0,
                view: view_i0,
            };

            // Based on O4 of (https://eprint.iacr.org/2017/279.pdf)
            let cm_i0 = pi0_execution.commit::<D>()?;

            let pi1_execution = PartyExecution {
                key: &k_i1,
                view: view_i1,
            };

            // Based on O4 of (https://eprint.iacr.org/2017/279.pdf)
            let cm_i1 = pi1_execution.commit::<D>()?;

            let cm_i2 = &proof.commitments[repetition];

            match party_index {
                0 => {
                    all_commitments.push(cm_i0);
                    all_commitments.push(cm_i1);
                    all_commitments.push(cm_i2.clone());

                    outputs.push(o0);
                    outputs.push(o1);
                    outputs.push(o2);
                }
                1 => {
                    all_commitments.push(cm_i2.clone());
                    all_commitments.push(cm_i0);
                    all_commitments.push(cm_i1);

                    outputs.push(o2);
                    outputs.push(o0);
                    outputs.push(o1);
                }
                2 => {
                    all_commitments.push(cm_i1);
                    all_commitments.push(cm_i2.clone());
                    all_commitments.push(cm_i0);

                    outputs.push(o1);
                    outputs.push(o2);
                    outputs.push(o0);
                }
                _ => panic!("Not trit"),
            };
        }

        let opening_indices = self.challenge.clone();
        if opening_indices != proof.claimed_trits {
            return Err(Error::FiatShamirOutputsMatchingError);
        }

        let _ = all_commitments
            .iter()
            .zip(self.all_commitments.iter())
            .map(|(a, b)| {
                if a.data != b.data {
                    return Err(Error::VerificationError);
                } else {
                    Ok(())
                }
            });
        if outputs != self.outputs {
            return Err(Error::VerificationError);
        }

        Ok(())
    }

    pub fn derive_third_output(
        public_output: &[GF2Word<T>],
        circuit: &impl Circuit<T>,
        circuit_simulation_output: (&Vec<GF2Word<T>>, &Vec<GF2Word<T>>),
    ) -> Vec<GF2Word<T>> {
        let party_output_len = circuit.party_output_len();
        let (o1, o2) = circuit_simulation_output;

        // TODO: introduce error here
        assert_eq!(o1.len(), party_output_len);
        assert_eq!(o2.len(), party_output_len);

        let mut derived_output = Vec::with_capacity(party_output_len);

        for i in 0..party_output_len {
            derived_output.push(o1[i] ^ o2[i] ^ public_output[i]);
        }

        derived_output
    }
}
