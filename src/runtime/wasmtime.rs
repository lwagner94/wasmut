use wasmtime::{Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::sync::WasiCtxBuilder;

use wasmtime_wasi::WasiCtx;

use anyhow::Result;

use crate::runtime::Runtime;
use crate::TestFunction;

pub struct WasmtimeRuntime {
    instance: Instance,
    store: Store<WasiCtx>,
}

impl Runtime for WasmtimeRuntime {
    fn new(bytecode: &[u8]) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::new(&engine, &bytecode)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_args()?
            .build();

        let mut store = Store::new(&engine, wasi);

        let instance = linker.instantiate(&mut store, &module)?;

        Ok(WasmtimeRuntime { instance, store })
    }

    fn call_returning_i32(&mut self, name: &str) -> Result<i32> {
        let func = self
            .instance
            .get_typed_func::<(), i32, _>(&mut self.store, name)?;

        Ok(func.call(&mut self.store, ())?)
    }

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>> {
        let function_names = self
            .instance
            .exports(&mut self.store)
            .filter_map(|export| {
                let name = String::from(export.name());
                export.into_func().map(|_| name)
            })
            .collect::<Vec<_>>();

        let test_functions = function_names
            .iter()
            .filter_map(|name| {
                self.instance
                    .get_typed_func::<(), i32, _>(&mut self.store, name)
                    .ok()
                    .map(|_| TestFunction { name: name.clone() })
            })
            .collect::<Vec<_>>();

        Ok(test_functions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::read;

    // TODO: See if it makes sense to generalize tests for both runtimes?

    #[test]
    fn test_simple_add() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmtimeRuntime::new(&bytecode)?;
        let result = runtime.call_returning_i32("test_add_1")?;
        assert_eq!(result, 1);
        Ok(())
    }

    #[test]
    fn test_discover_test_functions() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmtimeRuntime::new(&bytecode)?;
        let test_functions = runtime.discover_test_functions()?;
        assert_eq!(test_functions.len(), 2);
        Ok(())
    }
}
