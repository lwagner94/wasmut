pub mod wasmer;

use crate::policy::ExecutionPolicy;
use crate::wasmmodule::WasmModule;

use anyhow::Result;

use self::wasmer::WasmerRuntime;

pub enum ExecutionResult {
    // Normal termination
    ProcessExit { exit_code: u32, execution_cost: u64 },
    // Execution limit exceeded
    Timeout,

    // Other error
    Error,
}

pub trait Runtime {
    fn new(
        module: &WasmModule,
        discard_output: bool,
        map_dirs: &[(String, String)],
    ) -> Result<Self>
    where
        Self: Sized;

    fn call_test_function(&mut self, policy: ExecutionPolicy) -> Result<ExecutionResult>;
}

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
