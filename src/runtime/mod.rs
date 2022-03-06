pub mod wasmer;

use crate::wasmmodule::WasmModule;

/// Result of an executed module
pub enum ExecutionResult {
    /// Normal termination
    ProcessExit { exit_code: u32, execution_cost: u64 },
    /// Execution limit exceeded
    Timeout,

    /// Execution was skipped
    Skipped,

    /// Other error (e.g. module trapped)
    Error,
}
