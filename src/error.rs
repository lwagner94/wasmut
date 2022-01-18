use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error")]
    IOError { source: std::io::Error },

    #[error("bytecode deserialization failed")]
    BytecodeDeserialization {
        source: parity_wasm::elements::Error,
    },

    #[error("bytecode serialization failed")]
    BytecodeSerialization {
        source: parity_wasm::elements::Error,
    },
    #[error("runtime creation failed")]
    RuntimeCreation {
        source: anyhow::Error, // TODO: Is this clean?
    },

    #[error("runtime not available")]
    RuntimeNotAvailable,

    #[error("runtime call failed")]
    RuntimeCall {
        source: anyhow::Error, // TODO: Is this clean?
    },

    #[error("runtime execution trapped")]
    RuntimeTrap,

    #[error("regex creation failed")] // TODO
    RegexError(#[from] regex::Error),

    #[error("unknown error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
