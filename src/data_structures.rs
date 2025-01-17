use serde::Serialize;
use sha3::Digest;

use crate::{
    commitment::{Blinding, Commitment},
    error::Error,
    gf2_word::{GF2Word, Value},
    key::Key,
    view::View,
};

#[derive(Serialize)]
pub struct PartyExecution<'a, T: Value> {
    pub key: &'a Key,
    pub view: &'a View<T>,
}

/*
   Based on: O4 of (https://eprint.iacr.org/2017/279.pdf)
*/
impl<'a, T: Value> PartyExecution<'a, T> {
    pub fn commit<D: Default + Digest + Clone>(&self) -> Result<Commitment<D>, Error> {
        let blinding = Blinding(self.key);
        let messages_bytes: Vec<u8> = self
            .view
            .messages
            .iter()
            .flat_map(|msg| msg.value.to_bytes())
            .collect();

        // TODO: consider more optimal way to prepare message for committing
        // we omit commiting to full view to make sure that offset is not included which is just helper variable
        let commitment = Commitment::<D>::commit(
            &blinding,
            &[self.view.input.clone(), messages_bytes].concat(),
        )?;
        Ok(commitment)
    }
}

#[derive(Serialize)]
pub struct PublicInput<'a, T: Value> {
    pub hash_len: usize,
    pub security_param: usize,
    pub public_output: &'a Vec<GF2Word<T>>,
    pub outputs: &'a Vec<Vec<GF2Word<T>>>,
}

// TODO: add methods for computing proofs size, etc.
pub struct Proof<T: Value, D, const SIGMA: usize>
where
    D: Default + Digest + Clone,
{
    pub party_inputs: Vec<Vec<u8>>,
    pub commitments: Vec<Commitment<D>>,
    pub views: Vec<View<T>>,
    pub keys: Vec<Key>,
    pub claimed_trits: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct FirstMessageA<T: Value, D: Default + Digest + Clone> {
    pub outputs: Vec<Vec<GF2Word<T>>>,
    pub all_commitments: Vec<Commitment<D>>,
}
