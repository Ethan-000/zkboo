use crate::gf2_word::{GF2Word, Value};

pub fn generic_parse<T: Value + std::marker::Sync + std::marker::Send>(
    bytes: &[u8],
    number_of_words: usize,
) -> Vec<GF2Word<T>> {
    assert_eq!(bytes.len(), number_of_words * T::bytes_len());
    bytes
        .chunks(T::bytes_len())
        .map(|chunk| T::from_le_bytes(chunk).into())
        .collect()
}
