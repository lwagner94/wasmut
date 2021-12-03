use wasmtime::{Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::sync::WasiCtxBuilder;

use wasmtime_wasi::WasiCtx;

use crate::error::{Error, Result};

use crate::runtime::Runtime;
use crate::TestFunction;

use super::WasmModule;

pub struct WasmtimeRuntime {
    instance: Instance,
    store: Store<WasiCtx>,
}

impl Runtime for WasmtimeRuntime {
    fn new(module: WasmModule) -> Result<Self> {
        let engine = Engine::default();
        let bytecode: Vec<u8> = module.try_into()?;

        let module =
            Module::new(&engine, &bytecode).map_err(|e| Error::RuntimeCreation { source: e })?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .map_err(|e| Error::RuntimeCreation { source: e })?;

        let wasi = WasiCtxBuilder::new().inherit_stdio().build();

        let mut store = Store::new(&engine, wasi);

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::RuntimeCreation { source: e })?;

        Ok(WasmtimeRuntime { instance, store })
    }

    fn call_returning_i32(&mut self, name: &str) -> Result<i32> {
        let func = self
            .instance
            .get_typed_func::<(), i32, _>(&mut self.store, name)
            .map_err(|e| Error::RuntimeCall { source: e })?;

        func.call(&mut self.store, ())
            .map_err(|e| Error::RuntimeCall { source: e.into() })
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
