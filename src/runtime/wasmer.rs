use std::sync::Arc;

use wasmer::wasmparser::Operator;
use wasmer::CompilerConfig;
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_middlewares::{
    metering::{get_remaining_points, set_remaining_points, MeteringPoints},
    Metering,
};
use wasmer_wasi::WasiState;

use crate::{
    error::{Error, Result},
    ExecutionPolicy, ExecutionResult,
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

    fn call_returning_i32(&mut self, name: &str) -> Result<i32> {
        let func = self
            .instance
            .exports
            .get_function(name)
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        let native_func = func
            .native::<(), i32>()
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        native_func
            .call()
            .map_err(|e| Error::RuntimeCall { source: e.into() })
    }

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>> {
        let mut test_functions = Vec::new();

        for (name, func) in self.instance.exports.iter() {
            if let wasmer::Extern::Function(f) = func {
                if f.native::<(), i32>().is_ok() {
                    test_functions.push(TestFunction {
                        name: name.clone(),
                        expected_result: true,
                    });
                }
            }
        }
        Ok(test_functions)
    }

    fn call_test_function(
        &mut self,
        test_function: &TestFunction,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionResult<i32>> {
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

        let native_func = func
            .native::<(), i32>()
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        match native_func.call() {
            Ok(result) => {
                let cost = if let MeteringPoints::Remaining(remaining) =
                    get_remaining_points(&self.instance)
                {
                    u64::MAX - remaining
                } else {
                    // TODO: Can this be cleaner?
                    u64::MAX
                };

                Ok(ExecutionResult::Normal {
                    return_value: result,
                    cost,
                })
            }
            Err(_) => {
                // TODO: Trap reason

                match get_remaining_points(&self.instance) {
                    MeteringPoints::Exhausted => Ok(ExecutionResult::LimitExceeded),
                    _ => Ok(ExecutionResult::Error),
                }
            }
        }
    }
}
