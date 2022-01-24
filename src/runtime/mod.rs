pub mod wasmer;

use crate::policy::ExecutionPolicy;
use crate::wasmmodule::WasmModule;

use crate::error::Result;

use crate::ExecutionResult;

use self::wasmer::WasmerRuntime;

pub trait Runtime {
    fn new(module: WasmModule) -> Result<Self>
    where
        Self: Sized;

    fn call_test_function(&mut self, policy: ExecutionPolicy) -> Result<ExecutionResult>;
}

pub fn create_runtime(module: WasmModule) -> Result<Box<dyn Runtime>> {
    Ok(Box::new(WasmerRuntime::new(module)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::wasmer::WasmerRuntime;

    #[test]
    fn test_run_entry_point() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module)?;

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
        let mut runtime = WasmerRuntime::new(module)?;

        let result = runtime.call_test_function(ExecutionPolicy::RunUntilLimit { limit: 1 })?;

        assert!(matches!(result, ExecutionResult::LimitExceeded));

        Ok(())
    }
}
