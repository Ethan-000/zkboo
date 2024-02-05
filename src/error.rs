use thiserror::Error;
#[derive(Error, Debug)]
pub enum Error {
    #[error("zkboo serialization error")]
    SerializationError,
    #[error("zkboo hash length error")]
    HashLenError(usize, usize),
    #[error("zkboo verification error")]
    VerificationError,
    #[error("zkboo output reconstruction error")]
    OutputReconstructionError,
    #[error("zkboo fiat shamir error")]
    FiatShamirOutputsMatchingError,
    #[error("zkboo bit error")]
    BitError,
}
