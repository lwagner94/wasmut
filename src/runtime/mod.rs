pub mod wasmer;

use crate::policy::ExecutionPolicy;
use crate::wasmmodule::WasmModule;

use crate::error::Result;

use crate::{ExecutionResult, TestFunction};

use self::wasmer::WasmerRuntime;

pub trait Runtime {
    fn new(module: WasmModule) -> Result<Self>
    where
        Self: Sized;

    fn call_test_function(
        &mut self,
        test_function: &TestFunction,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionResult>;

    fn discover_test_functions(&mut self) -> Option<Vec<TestFunction>>;
    fn discover_entry_point(&mut self) -> Option<TestFunction>;
}

pub fn create_runtime(module: WasmModule) -> Result<Box<dyn Runtime>> {
    Ok(Box::new(WasmerRuntime::new(module)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::wasmer::WasmerRuntime;

    #[test]
    fn test_discover_test_functions() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module)?;
        let test_functions = runtime.discover_test_functions().unwrap();
        assert_eq!(test_functions.len(), 2);
        assert!(test_functions
            .iter()
            .all(|f| { f.name.starts_with("test_") }));

        Ok(())
    }

    #[test]
    fn test_discover_entry_point() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module)?;
        let test_function = runtime.discover_entry_point().unwrap();
        assert!(test_function.name == "_start");

        Ok(())
    }

    #[test]
    fn test_run_all_tests() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module)?;
        let test_functions = runtime.discover_test_functions().unwrap();

        for test_function in test_functions {
            let result =
                runtime.call_test_function(&test_function, ExecutionPolicy::RunUntilReturn)?;
            assert!(matches!(
                result,
                ExecutionResult::FunctionReturn {
                    // cost: 18,
                    return_value: 1
                }
            ));
        }
        Ok(())
    }

    #[test]
    fn test_run_entry_point() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module)?;
        let test_function = runtime.discover_entry_point().unwrap();

        let result = runtime.call_test_function(&test_function, ExecutionPolicy::RunUntilReturn)?;
        assert!(matches!(
            result,
            ExecutionResult::ProcessExit {
                // cost: 18,
                exit_code: 0
            }
        ));

        Ok(())
    }

    #[test]
    fn test_execution_limit() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(module)?;
        let test_functions = runtime.discover_test_functions().unwrap();

        for test_function in test_functions {
            let result = runtime
                .call_test_function(&test_function, ExecutionPolicy::RunUntilLimit { limit: 1 })?;

            assert!(matches!(result, ExecutionResult::LimitExceeded));
        }

        Ok(())
    }
}
