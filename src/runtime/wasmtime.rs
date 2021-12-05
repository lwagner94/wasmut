use wasmtime::{Config, Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::sync::WasiCtxBuilder;

use wasmtime_wasi::WasiCtx;

use crate::error::{Error, Result};

use crate::runtime::Runtime;
use crate::{ExecutionPolicy, ExecutionResult, TestFunction};

use super::WasmModule;

pub struct WasmtimeRuntime {
    instance: Instance,
    store: Store<WasiCtx>,
}

impl Runtime for WasmtimeRuntime {
    fn new(module: WasmModule) -> Result<Self> {
        let mut config = Config::default();

        config.consume_fuel(true);

        let engine = Engine::new(&config).map_err(|e| Error::RuntimeCreation { source: e })?;
        let bytecode: Vec<u8> = module.try_into()?;

        let module =
            Module::new(&engine, &bytecode).map_err(|e| Error::RuntimeCreation { source: e })?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .map_err(|e| Error::RuntimeCreation { source: e })?;

        let wasi = WasiCtxBuilder::new().inherit_stdio().build();

        let mut store = Store::new(&engine, wasi);
        store.add_fuel(1).unwrap();

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::RuntimeCreation { source: e })?;

        Ok(WasmtimeRuntime { instance, store })
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
                    .map(|_| TestFunction {
                        name: name.clone(),
                        expected_result: true,
                    })
            })
            .collect::<Vec<_>>();

        Ok(test_functions)
    }

    fn call_test_function(
        &mut self,
        test_function: &TestFunction,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionResult<i32>> {
        // Ad TODO: Whenever we consume fuel, the amount of consumed is added
        // to a variable, which may not overflow. so we cannot use u64::MAX here.
        let limit = match policy {
            ExecutionPolicy::RunUntilLimit { limit } => limit,
            ExecutionPolicy::RunUntilReturn => u32::MAX as u64, // TODO
        };

        // We always leave 1 unit of fuel in the store
        self.store.add_fuel(limit - 1).unwrap();

        let name = test_function.name.as_str();

        let func = self
            .instance
            .get_typed_func::<(), i32, _>(&mut self.store, name)
            .map_err(|e| Error::RuntimeCall { source: e })?;

        let consumed_fuel_before = self.store.fuel_consumed().unwrap();
        let result = func.call(&mut self.store, ());
        let consumed_fuel_after = self.store.fuel_consumed().unwrap();
        let consumed_fuel = consumed_fuel_after - consumed_fuel_before;

        if consumed_fuel >= limit {
            self.store.add_fuel(1).unwrap();
        } else {
            let leftover = self.store.consume_fuel(0).unwrap();
            self.store.consume_fuel(leftover - 1).unwrap();
        }

        match result {
            Ok(return_value) => Ok(ExecutionResult::Normal {
                return_value,
                cost: consumed_fuel,
            }),
            Err(e) => {
                // TODO: Trap reason
                // let ne: Box<dyn std::error::Error> = e.into();
                // dbg!(ne.source());

                if consumed_fuel >= limit {
                    Ok(ExecutionResult::LimitExceeded)
                } else {
                    // TODO: Handle other errors
                    dbg!(e);
                    Ok(ExecutionResult::Error)
                }
            }
        }
    }
}
