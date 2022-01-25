pub mod addressresolver;
pub mod config;
pub mod defaults;
pub mod error;
pub mod executor;
pub mod mutation;
pub mod operator;
pub mod policy;
pub mod reporter;
pub mod runtime;
pub mod wasmmodule;

#[derive(Debug)]
pub enum ExecutionResult {
    // Normal termination
    ProcessExit { exit_code: u32, execution_cost: u64 },
    // Execution limit exceeded
    LimitExceeded,

    // Other error
    Error,
}

#[cfg(test)]
mod tests {}
