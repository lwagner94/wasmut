use wasmtime::{Config, Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::sync::WasiCtxBuilder;

use wasmtime_wasi::WasiCtx;

use crate::error::{Error, Result};

use crate::runtime::Runtime;
use crate::{ExecutionPolicy, ExecutionResult, TestFunction, TestFunctionType};

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

    fn call_test_function(
        &mut self,
        test_function: &TestFunction,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionResult> {
        // Ad TODO: Whenever we consume fuel, the amount of consumed is added
        // to a variable, which may not overflow. so we cannot use u64::MAX here.
        let limit = match policy {
            ExecutionPolicy::RunUntilLimit { limit } => limit,
            ExecutionPolicy::RunUntilReturn => u32::MAX as u64, // TODO
        };

        // We always leave 1 unit of fuel in the store
        self.store.add_fuel(limit - 1).unwrap();

        let name = test_function.name.as_str();

        let consumed_fuel_before = self.store.fuel_consumed().unwrap();

        let result = match test_function.function_type {
            TestFunctionType::FuncReturningI32 => {
                let func = self
                    .instance
                    .get_typed_func::<(), i32, _>(&mut self.store, name)
                    .map_err(|e| Error::RuntimeCall { source: e })?;

                func.call(&mut self.store, ())
            }
            TestFunctionType::StartEntryPoint => {
                let func = self
                    .instance
                    .get_typed_func::<(), (), _>(&mut self.store, name)
                    .map_err(|e| Error::RuntimeCall { source: e })?;

                // Map to Result<i32> so that the type matches
                func.call(&mut self.store, ()).map(|_| 0)
            }
        };

        let consumed_fuel_after = self.store.fuel_consumed().unwrap();
        let consumed_fuel = consumed_fuel_after - consumed_fuel_before;

        if consumed_fuel >= limit {
            self.store.add_fuel(1).unwrap();
        } else {
            let leftover = self.store.consume_fuel(0).unwrap();
            self.store.consume_fuel(leftover - 1).unwrap();
        }

        match result {
            Ok(return_value) => match test_function.function_type {
                TestFunctionType::FuncReturningI32 => Ok(ExecutionResult::FunctionReturn {
                    return_value,
                    // cost: consumed_fuel,
                }),
                TestFunctionType::StartEntryPoint => Ok(ExecutionResult::ProcessExit {
                    exit_code: return_value as u32,
                    // cost: consumed_fuel,
                }),
            },
            Err(e) => {
                if consumed_fuel >= limit {
                    Ok(ExecutionResult::LimitExceeded)
                } else {
                    // TODO: Handle other errors

                    if let Some(code) = e.i32_exit_status() {
                        Ok(ExecutionResult::ProcessExit {
                            exit_code: code as u32,
                        })
                    } else {
                        Ok(ExecutionResult::Error)
                    }
                }
            }
        }
    }

    fn discover_test_functions(&mut self) -> Option<Vec<TestFunction>> {
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
                        function_type: TestFunctionType::FuncReturningI32,
                    })
            })
            .collect::<Vec<_>>();

        Some(test_functions)
    }

    fn discover_entry_point(&mut self) -> Option<TestFunction> {
        self.instance.exports(&mut self.store).find_map(|export| {
            let name = String::from(export.name());
            if name == "_start" {
                export.into_func().map(|_| TestFunction {
                    name,
                    expected_result: false,
                    function_type: TestFunctionType::StartEntryPoint,
                })
            } else {
                None
            }
        })
    }
}
