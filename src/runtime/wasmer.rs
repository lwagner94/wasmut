use std::sync::Arc;

use wasmer::wasmparser::Operator;
use wasmer::CompilerConfig;
use wasmer_compiler_singlepass::Singlepass;
use wasmer_engine_universal::Universal;
use wasmer_middlewares::{
    metering::{get_remaining_points, set_remaining_points, MeteringPoints},
    Metering,
};
use wasmer_wasi::{Pipe, WasiError, WasiState};

use crate::runtime::Runtime;
use crate::{
    error::{Error, Result},
    policy::ExecutionPolicy,
    ExecutionResult,
};

use super::WasmModule;

pub struct WasmerRuntime {
    instance: wasmer::Instance,
}

impl Runtime for WasmerRuntime {
    fn new(module: WasmModule) -> Result<Self> {
        use wasmer::{Instance, Module, Store};

        let cost_function = |_: &Operator| -> u64 { 1 };

        let metering = Arc::new(Metering::new(u64::MAX, cost_function));
        let mut compiler_config = Singlepass::default();
        compiler_config.push_middleware(metering);

        let store = Store::new(&Universal::new(compiler_config).engine());
        let bytecode: Vec<u8> = module.try_into()?;
        let module = Module::new(&store, &bytecode)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        let stdout = Box::new(Pipe::new());
        let stderr = Box::new(Pipe::new());

        let mut wasi_env = WasiState::new("command-name")
            .stdout(stdout)
            .stderr(stderr)
            .finalize()
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        let import_object = wasi_env
            .import_object(&module)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;
        let instance = Instance::new(&module, &import_object)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        Ok(WasmerRuntime { instance })
    }

    fn call_test_function(&mut self, policy: ExecutionPolicy) -> Result<ExecutionResult> {
        let execution_limit = match policy {
            ExecutionPolicy::RunUntilLimit { limit } => limit,
            ExecutionPolicy::RunUntilReturn => u64::MAX,
        };

        set_remaining_points(&self.instance, execution_limit);

        let func = self
            .instance
            .exports
            .get_function("_start")
            .map_err(|e| Error::RuntimeCall { source: e.into() })?
            .native::<(), ()>()
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        let result = func.call().map(|_| 0);

        match result {
            Ok(result) => {
                let execution_cost = if let MeteringPoints::Remaining(remaining) =
                    get_remaining_points(&self.instance)
                {
                    execution_limit - remaining
                } else {
                    // TODO: Can this be cleaner?
                    execution_limit
                };

                Ok(ExecutionResult::ProcessExit {
                    exit_code: result as u32,
                    execution_cost,
                })
            }
            Err(e) => {
                match get_remaining_points(&self.instance) {
                    MeteringPoints::Exhausted => Ok(ExecutionResult::LimitExceeded),
                    MeteringPoints::Remaining(remaining) => {
                        // use std::error::Error;
                        // if let Some(err) = e.source() {

                        // } else {
                        //     Ok(ExecutionResult::Error)
                        // }
                        // dbg!(&e);
                        if let Ok(wasi_err) = e.downcast() {
                            match wasi_err {
                                WasiError::Exit(exit_code) => {
                                    let execution_cost = execution_limit - remaining;

                                    Ok(ExecutionResult::ProcessExit {
                                        exit_code,
                                        execution_cost,
                                    })
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
}
