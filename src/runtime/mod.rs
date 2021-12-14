#[cfg(feature = "runtime-wasmer")]
pub mod wasmer;
#[cfg(feature = "runtime-wasmtime")]
pub mod wasmtime;

#[cfg(feature = "runtime-wasmer")]
use crate::runtime::wasmer::WasmerRuntime;
#[cfg(feature = "runtime-wasmtime")]
use crate::runtime::wasmtime::WasmtimeRuntime;

use crate::wasmmodule::WasmModule;
use crate::policy::ExecutionPolicy;

use crate::error::{Result, Error};

use crate::{ExecutionResult, TestFunction};

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

#[derive(Debug, Copy, Clone)]
pub enum RuntimeType {
    Wasmtime,
    Wasmer,
}

use RuntimeType::*;

#[allow(unreachable_patterns)]
pub fn create_runtime(rt: RuntimeType, module: WasmModule) -> Result<Box<dyn Runtime>> {
    match rt {
        
        #[cfg(feature = "runtime-wasmtime")] Wasmtime => Ok(Box::new(WasmtimeRuntime::new(module)?)),
        #[cfg(feature = "runtime-wasmer")] Wasmer => Ok(Box::new(WasmerRuntime::new(module)?)),
        _ => Err(Error::RuntimeNotAvailable)
    }
}

pub fn get_runtime_types() -> Vec<RuntimeType> {
    let mut runtimes = Vec::new();
    #[cfg(feature = "runtime-wasmtime")]
    runtimes.push(Wasmtime);
    #[cfg(feature = "runtime-wasmer")]
    runtimes.push(Wasmer);
    runtimes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_test_functions() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_functions = runtime.discover_test_functions().unwrap();
            assert_eq!(test_functions.len(), 2);
            assert!(test_functions
                .iter()
                .all(|f| { f.name.starts_with("test_") }));
        }

        Ok(())
    }

    #[test]
    fn test_discover_entry_point() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_function = runtime.discover_entry_point().unwrap();
            assert!(test_function.name == "_start");
        }

        Ok(())
    }

    #[test]
    fn test_run_all_tests() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
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
        }

        Ok(())
    }

    #[test]
    fn test_run_entry_point() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_function = runtime.discover_entry_point().unwrap();

            let result =
                runtime.call_test_function(&test_function, ExecutionPolicy::RunUntilReturn)?;
            assert!(matches!(
                result,
                ExecutionResult::ProcessExit {
                    // cost: 18,
                    exit_code: 0
                }
            ));
        }

        Ok(())
    }

    #[test]
    fn test_execution_limit() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_functions = runtime.discover_test_functions().unwrap();

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
