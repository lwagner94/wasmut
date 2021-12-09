use std::sync::Arc;

use wasmer::wasmparser::Operator;
use wasmer::CompilerConfig;
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_middlewares::{
    metering::{get_remaining_points, set_remaining_points, MeteringPoints},
    Metering,
};
use wasmer_wasi::{WasiError, WasiState};

use crate::{
    error::{Error, Result},
    policy::ExecutionPolicy,
    ExecutionResult, TestFunctionType,
};
use crate::{runtime::Runtime, TestFunction};

use super::WasmModule;

pub struct WasmerRuntime {
    instance: wasmer::Instance,
}

impl Runtime for WasmerRuntime {
    fn new(module: WasmModule) -> Result<Self> {
        use wasmer::{Instance, Module, Store};

        let cost_function = |_: &Operator| -> u64 { 1 };

        let metering = Arc::new(Metering::new(u64::MAX, cost_function));
        let mut compiler_config = Cranelift::default();
        compiler_config.push_middleware(metering);

        let store = Store::new(&Universal::new(compiler_config).engine());
        let bytecode: Vec<u8> = module.try_into()?;
        let module = Module::new(&store, &bytecode)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        let mut wasi_env = WasiState::new("command-name")
            .finalize()
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        let import_object = wasi_env
            .import_object(&module)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;
        let instance = Instance::new(&module, &import_object)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        Ok(WasmerRuntime { instance })
    }

    fn call_test_function(
        &mut self,
        test_function: &TestFunction,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionResult> {
        let name = test_function.name.as_str();

        match policy {
            ExecutionPolicy::RunUntilLimit { limit } => set_remaining_points(&self.instance, limit),
            ExecutionPolicy::RunUntilReturn => set_remaining_points(&self.instance, u64::MAX),
        }

        let func = self
            .instance
            .exports
            .get_function(name)
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        let result = match test_function.function_type {
            TestFunctionType::FuncReturningI32 => {
                let native_func = func
                    .native::<(), i32>()
                    .map_err(|e| Error::RuntimeCall { source: e.into() })?;
                native_func.call()
            }
            TestFunctionType::StartEntryPoint => {
                let native_func = func
                    .native::<(), ()>()
                    .map_err(|e| Error::RuntimeCall { source: e.into() })?;
                native_func.call().map(|_| 0)
            }
        };

        match result {
            Ok(result) => {
                let _cost = if let MeteringPoints::Remaining(remaining) =
                    get_remaining_points(&self.instance)
                {
                    u64::MAX - remaining
                } else {
                    // TODO: Can this be cleaner?
                    u64::MAX
                };

                match test_function.function_type {
                    TestFunctionType::StartEntryPoint => Ok(ExecutionResult::ProcessExit {
                        exit_code: result as u32,
                    }),
                    TestFunctionType::FuncReturningI32 => Ok(ExecutionResult::FunctionReturn {
                        return_value: result,
                    }),
                }
            }
            Err(e) => {
                match get_remaining_points(&self.instance) {
                    MeteringPoints::Exhausted => Ok(ExecutionResult::LimitExceeded),
                    MeteringPoints::Remaining(_remaining) => {
                        // use std::error::Error;
                        // if let Some(err) = e.source() {

                        // } else {
                        //     Ok(ExecutionResult::Error)
                        // }
                        if let Ok(wasi_err) = e.downcast() {
                            dbg!(&wasi_err);
                            match wasi_err {
                                WasiError::Exit(exit_code) => {
                                    Ok(ExecutionResult::ProcessExit { exit_code })
                                }
                                WasiError::UnknownWasiVersion => Ok(ExecutionResult::Error),
                            }
                        } else {
                            Ok(ExecutionResult::Error)
                        }
                    }
                }
            }
        }
    }

    fn discover_test_functions(&mut self) -> Option<Vec<TestFunction>> {
        let mut test_functions = Vec::new();

        for (name, func) in self.instance.exports.iter() {
            if let wasmer::Extern::Function(f) = func {
                if f.native::<(), i32>().is_ok() {
                    test_functions.push(TestFunction {
                        name: name.clone(),
                        expected_result: true,
                        function_type: TestFunctionType::FuncReturningI32,
                    });
                }
            }
        }
        Some(test_functions)
    }

    fn discover_entry_point(&mut self) -> Option<TestFunction> {
        self.instance.exports.iter().find_map(|(name, func)| {
            if let wasmer::Extern::Function(f) = func {
                if name == "_start" && f.native::<(), ()>().is_ok() {
                    Some(TestFunction {
                        name: name.clone(),
                        expected_result: false,
                        function_type: TestFunctionType::StartEntryPoint,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
}
