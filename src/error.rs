use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),

    #[error("File \"{0}\" not found")]
    FileNotFoundError(String),

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

    #[error("configuration erorr")]
    ConfigError(#[from] toml::de::Error),

    #[error("Execution of module returned exit code {0}")]
    WasmModuleNonzeroExit(u32),

    #[error("Execution of module failed")]
    WasmModuleFailed,

    #[error("unknown error")]
    Unknown(#[source] anyhow::Error),

    #[error("Module does not have a code section")]
    WasmModuleNoCodeSection,
}

pub type Result<T> = std::result::Result<T, Error>;
