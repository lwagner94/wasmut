use std::sync::Arc;

use crate::{
    policy::ExecutionPolicy,
    runtime::{ExecutionResult, Runtime},
};
use anyhow::{Context, Result};
use wasmer::wasmparser::Operator;
use wasmer::CompilerConfig;
use wasmer_compiler_singlepass::Singlepass;
use wasmer_engine_universal::Universal;
use wasmer_middlewares::{
    metering::{get_remaining_points, set_remaining_points, MeteringPoints},
    Metering,
};
use wasmer_wasi::{Pipe, WasiError, WasiState};

use super::WasmModule;

pub struct WasmerRuntime {
    instance: wasmer::Instance,
}

impl Runtime for WasmerRuntime {
    fn new(module: WasmModule, discard_output: bool) -> Result<Self> {
        use wasmer::{Instance, Module, Store};

        let cost_function = |_: &Operator| -> u64 { 1 };

        let metering = Arc::new(Metering::new(u64::MAX, cost_function));
        let mut compiler_config = Singlepass::default();
        compiler_config.push_middleware(metering);

        let store = Store::new(&Universal::new(compiler_config).engine());
        let bytecode: Vec<u8> = module.try_into()?;
        let module = Module::new(&store, &bytecode).context("Failed to create wasmer module")?;

        let mut state_builder = WasiState::new("command-name");

        if discard_output {
            let stdout = Box::new(Pipe::new());
            let stderr = Box::new(Pipe::new());
            state_builder.stdout(stdout).stderr(stderr);
        }

        let mut wasi_env = state_builder
            .finalize()
            .context("Failed to create wasmer-wasi env")?;

        let import_object = wasi_env
            .import_object(&module)
            .context("Failed to create import object")?;
        let instance =
            Instance::new(&module, &import_object).context("Failed to create wasmer instance")?;

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
            .context("Failed to resolve _start function")?
            .native::<(), ()>()
            .context("Failed to get native _start function")?;

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
                    MeteringPoints::Exhausted => Ok(ExecutionResult::Timeout),
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
