pub mod wasmer;
pub mod wasmtime;
use crate::runtime::wasmer::WasmerRuntime;
use crate::runtime::wasmtime::WasmtimeRuntime;
use crate::wasmmodule::WasmModule;

use crate::error::Result;

use crate::TestFunction;

pub trait Runtime {
    fn new(module: WasmModule) -> Result<Self>
    where
        Self: Sized;
    fn call_returning_i32(&mut self, name: &str) -> Result<i32>;

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>>;
}

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
    fn test_simple_add() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let result = runtime.call_returning_i32("test_add_1")?;
            assert_eq!(result, 1);
        }

        Ok(())
    }

    #[test]
    fn test_discover_test_functions() -> Result<()> {
        for runtime_ty in get_runtime_types() {
            let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
            let mut runtime = create_runtime(runtime_ty, module)?;
            let test_functions = runtime.discover_test_functions()?;
            assert_eq!(test_functions.len(), 2);
        }

        Ok(())
    }
}
