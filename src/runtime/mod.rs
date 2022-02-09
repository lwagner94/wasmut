pub mod wasmer;

use crate::policy::ExecutionPolicy;
use crate::wasmmodule::WasmModule;

use anyhow::Result;

use self::wasmer::WasmerRuntime;

#[derive(Debug)]
pub enum ExecutionResult {
    // Normal termination
    ProcessExit { exit_code: u32, execution_cost: u64 },
    // Execution limit exceeded
    Timeout,

    // Other error
    Error,
}

pub trait Runtime {
    fn new(module: WasmModule, discard_output: bool, map_dirs: &[(String, String)]) -> Result<Self>
    where
        Self: Sized;

    fn call_test_function(&mut self, policy: ExecutionPolicy) -> Result<ExecutionResult>;
}

pub fn create_runtime(
    module: WasmModule,
    discard_output: bool,
    map_dirs: &[(String, String)],
) -> Result<Box<dyn Runtime>> {
    Ok(Box::new(WasmerRuntime::new(
        module,
        discard_output,
        map_dirs,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::wasmer::WasmerRuntime;

    #[test]
    fn test_run_entry_point() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module, true, &[])?;

        let result = runtime.call_test_function(ExecutionPolicy::RunUntilReturn)?;

        if let ExecutionResult::ProcessExit {
            exit_code,
            execution_cost,
        } = result
        {
            assert_eq!(exit_code, 0);
            assert!(execution_cost > 20);
            assert!(execution_cost < 60);
        } else {
            panic!();
        }

        Ok(())
    }

    #[test]
    fn test_execution_limit() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module, true, &[])?;

        let result = runtime.call_test_function(ExecutionPolicy::RunUntilLimit { limit: 1 })?;

        assert!(matches!(result, ExecutionResult::Timeout));

        Ok(())
    }
}
