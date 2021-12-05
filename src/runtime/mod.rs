pub mod wasmer;
pub mod wasmtime;
use crate::runtime::wasmer::WasmerRuntime;
use crate::runtime::wasmtime::WasmtimeRuntime;
use crate::wasmmodule::WasmModule;

use crate::error::Result;

use crate::{ExecutionPolicy, ExecutionResult, TestFunction};

pub trait Runtime {
    fn new(module: WasmModule) -> Result<Self>
    where
        Self: Sized;

    fn call_test_function(
        &mut self,
        test_function: &TestFunction,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionResult<i32>>;

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>>;
}

#[derive(Debug, Copy, Clone)]
pub enum RuntimeType {
    Wasmtime,
    Wasmer,
}

use RuntimeType::*;

pub fn create_runtime(rt: RuntimeType, module: WasmModule) -> Result<Box<dyn Runtime>> {
    match rt {
        Wasmtime => Ok(Box::new(WasmtimeRuntime::new(module)?)),
        Wasmer => Ok(Box::new(WasmerRuntime::new(module)?)),
    }
}

pub fn get_runtime_types() -> Vec<RuntimeType> {
    vec![Wasmtime, Wasmer]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_test_functions() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_functions = runtime.discover_test_functions()?;
            assert_eq!(test_functions.len(), 2);
            assert!(test_functions
                .iter()
                .all(|f| { f.name.starts_with("test_") }));
        }

        Ok(())
    }

    #[test]
    fn test_run_all_tests() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_functions = runtime.discover_test_functions()?;

            for test_function in test_functions {
                let result =
                    runtime.call_test_function(&test_function, ExecutionPolicy::RunUntilReturn)?;
                assert!(matches!(
                    result,
                    ExecutionResult::Normal {
                        cost: 18,
                        return_value: 1
                    }
                ));
            }
        }

        Ok(())
    }

    #[test]
    fn test_execution_limit() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_functions = runtime.discover_test_functions()?;

            for test_function in test_functions {
                let result = runtime.call_test_function(
                    &test_function,
                    ExecutionPolicy::RunUntilLimit { limit: 1 },
                )?;

                assert!(matches!(result, ExecutionResult::LimitExceeded));
            }
        }

        Ok(())
    }
}
