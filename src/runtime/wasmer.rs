use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{
    policy::ExecutionPolicy,
    runtime::{ExecutionResult, Runtime},
};
use anyhow::{Context, Result};
use wasmer::{wasmparser::Operator, Exports, ImportObject, Instance, Module, Store, WasmerEnv};
use wasmer::{CompilerConfig, Function};
use wasmer_compiler_singlepass::Singlepass;
use wasmer_engine_universal::Universal;
use wasmer_middlewares::{
    metering::{get_remaining_points, set_remaining_points, MeteringPoints},
    Metering,
};
use wasmer_wasi::{Pipe, WasiError, WasiState};

#[derive(WasmerEnv, Clone, Default)]
struct TraceEnv {
    points: Arc<Mutex<HashSet<u64>>>,
}

fn trace(env: &TraceEnv, address: i64) {
    let mut vec = env.points.lock().unwrap();
    vec.insert(address as u64);
}
use super::WasmModule;

pub struct WasmerRuntime {
    instance: wasmer::Instance,
    trace_env: Option<TraceEnv>,
}

impl Runtime for WasmerRuntime {
    fn new(
        module: &WasmModule,
        discard_output: bool,
        coverage: bool,
        map_dirs: &[(String, String)],
    ) -> Result<Self> {
        let store = create_store();
        let wasmer_module = create_module(module, &store)?;
        let mut import_object =
            create_wasi_import_object(discard_output, map_dirs, &wasmer_module)?;

        let trace_env = if coverage {
            Some(add_trace_function(store, &mut import_object))
        } else {
            None
        };

        Ok(WasmerRuntime {
            instance: Instance::new(&wasmer_module, &import_object)
                .context("Failed to create wasmer instance")?,
            trace_env,
        })
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
            Err(e) => match get_remaining_points(&self.instance) {
                MeteringPoints::Exhausted => Ok(ExecutionResult::Timeout),
                MeteringPoints::Remaining(remaining) => {
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
            },
        }
    }

    fn trace_points(&self) -> Option<HashSet<u64>> {
        self.trace_env.as_ref().map(|env| {
            let points = env.points.lock().unwrap();
            points.clone()
        })
    }
}

fn add_trace_function(store: Store, import_object: &mut ImportObject) -> TraceEnv {
    let mut exports = Exports::new();
    let trace_env: TraceEnv = Default::default();
    exports.insert(
        "__wasmut_trace",
        Function::new_native_with_env(&store, trace_env.clone(), trace),
    );
    import_object.register("wasmut_api", exports);
    trace_env
}

fn create_store() -> Store {
    // Define cost fuction for any executed instruction
    let cost_function = |_: &Operator| -> u64 { 1 };
    let metering = Arc::new(Metering::new(u64::MAX, cost_function));

    // We use the Singlepass compiler, because in general
    // we expect to have *many* mutants with little execution time
    // Compared to Cranelift oder LLVM, the Singlepass compiler
    // is very quick at the cost of increased execution cost of
    // the module.
    let mut compiler_config = Singlepass::default();
    compiler_config.push_middleware(metering);
    Store::new(&Universal::new(compiler_config).engine())
}

fn create_module(module: &WasmModule, store: &Store) -> Result<Module, anyhow::Error> {
    let bytecode: Vec<u8> = module.to_bytes()?;
    let module = Module::new(store, &bytecode).context("Failed to create wasmer module")?;

    Ok(module)
}

fn create_wasi_import_object(
    discard_output: bool,
    map_dirs: &[(String, String)],
    module: &Module,
) -> Result<ImportObject> {
    let mut state_builder = WasiState::new("command-name");

    // If the discard_output parameter is set, we discard any outputs of the module
    if discard_output {
        let stdout = Box::new(Pipe::new());
        let stderr = Box::new(Pipe::new());
        state_builder.stdout(stdout).stderr(stderr);
    }

    // Map directories to the virtual machine
    for mapped_dir in map_dirs {
        state_builder
            .map_dir(&mapped_dir.1, &mapped_dir.0)
            .with_context(|| format!("Could not map {} to {}", mapped_dir.0, mapped_dir.1))?;
    }

    let mut wasi_env = state_builder
        .finalize()
        .context("Failed to create wasmer-wasi env")?;

    let import_object = wasi_env
        .import_object(module)
        .context("Failed to create import object")?;

    Ok(import_object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::wasmer::WasmerRuntime;

    #[test]
    fn test_run_entry_point() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(&module, true, false, &[])?;

        let result = runtime.call_test_function(ExecutionPolicy::RunUntilReturn)?;

        if let ExecutionResult::ProcessExit {
            exit_code,
            execution_cost,
        } = result
        {
            assert_eq!(exit_code, 0);
            assert!(execution_cost > 20);
            assert!(execution_cost < 60);
        } else {
            panic!();
        }

        Ok(())
    }

    #[test]
    fn test_execution_limit() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(&module, true, false, &[])?;

        let result = runtime.call_test_function(ExecutionPolicy::RunUntilLimit { limit: 1 })?;

        assert!(matches!(result, ExecutionResult::Timeout));

        Ok(())
    }
}
