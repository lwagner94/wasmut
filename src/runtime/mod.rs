pub mod wasmer;

use crate::policy::ExecutionPolicy;
use crate::wasmmodule::WasmModule;

use anyhow::Result;

use self::wasmer::WasmerRuntime;

/// Result of an executed module
pub enum ExecutionResult {
    /// Normal termination
    ProcessExit { exit_code: u32, execution_cost: u64 },
    /// Execution limit exceeded
    Timeout,

    /// Other error (e.g. module trapped)
    Error,
}

/// This trait represents a Runtime implementation
pub trait Runtime {
    /// Create a new runtime instance
    fn new(
        module: &WasmModule,
        discard_output: bool,
        map_dirs: &[(String, String)],
    ) -> Result<Self>
    where
        Self: Sized;

    /// Call the _start entry point of the module
    fn call_test_function(&mut self, policy: ExecutionPolicy) -> Result<ExecutionResult>;
}

/// Utility function used to create a new runtime.
pub fn create_runtime(
    module: &WasmModule,
    discard_output: bool,
    map_dirs: &[(String, String)],
) -> Result<Box<dyn Runtime>> {
    Ok(Box::new(WasmerRuntime::new(
        module,
        discard_output,
        map_dirs,
    )?))
}
