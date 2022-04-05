use std::fmt::Display;
use std::sync::{Arc, Mutex};

use crate::{policy::ExecutionPolicy, runtime::ExecutionResult};
use anyhow::{Context, Result};
use wasmer::{wasmparser::Operator, Exports, ImportObject, Instance, Module, Store, WasmerEnv};
use wasmer::{CompilerConfig, Cranelift, Function};
use wasmer_compiler_singlepass::Singlepass;
use wasmer_engine_universal::Universal;
use wasmer_middlewares::{
    metering::{get_remaining_points, set_remaining_points, MeteringPoints},
    Metering,
};
use wasmer_wasi::{Pipe, WasiError, WasiState};

#[derive(Copy, Clone)]
pub enum Compiler {
    Singlepass,
    Cranelift,
}

impl Display for Compiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compiler::Singlepass => write!(f, "Singlepass"),
            Compiler::Cranelift => write!(f, "Cranelift"),
        }
    }
}

#[derive(WasmerEnv, Clone, Default)]
struct MutantEnv {
    points: Arc<Mutex<TracePoints>>,
    activated_mutant_id: i64,
}

impl MutantEnv {
    fn new(activated_mutant_id: i64) -> Self {
        Self {
            points: Default::default(),
            activated_mutant_id,
        }
    }
}

fn trace(env: &MutantEnv, address: i64) {
    let mut vec = env.points.lock().unwrap();
    vec.add_point(address as u64);
}

fn check_mutant_id(env: &MutantEnv, mutant_id: i64) -> i32 {
    if env.activated_mutant_id == mutant_id {
        1
    } else {
        0
    }
}

use super::{TracePoints, WasmModule};

pub struct WasmerRuntime {
    instance: wasmer::Instance,
    mutant_env: MutantEnv,
    compiler: Compiler,
}

impl WasmerRuntime {
    pub fn new(
        module: &WasmModule,
        discard_output: bool,
        map_dirs: &[(String, String)],
    ) -> Result<Self> {
        let store = create_store(Compiler::Singlepass);
        let trace_env = MutantEnv::new(0);

        let wasmer_module = create_module(module, &store)?;
        let mut import_object =
            create_wasi_import_object(discard_output, map_dirs, &wasmer_module)?;
        add_trace_function(&store, &mut import_object, &trace_env);

        Ok(WasmerRuntime {
            instance: Instance::new(&wasmer_module, &import_object)
                .context("Failed to create wasmer instance")?,
            mutant_env: trace_env,
            compiler: Compiler::Singlepass,
        })
    }

    fn new_from_cached_module(
        compiled_code: &[u8],
        discard_output: bool,
        map_dirs: &[(String, String)],
        mutant_id: i64,
        compiler: Compiler,
    ) -> Result<Self> {
        let store = create_store(compiler);
        let mutant_env = MutantEnv::new(mutant_id);

        let wasmer_module = unsafe { Module::deserialize(&store, compiled_code)? };

        let mut import_object =
            create_wasi_import_object(discard_output, map_dirs, &wasmer_module)?;
        add_trace_function(&store, &mut import_object, &mutant_env);

        Ok(WasmerRuntime {
            instance: Instance::new(&wasmer_module, &import_object)
                .context("Failed to create wasmer instance")?,
            mutant_env,
            compiler,
        })
    }

    pub fn call_test_function(&mut self, policy: ExecutionPolicy) -> Result<ExecutionResult> {
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

    pub fn trace_points(&self) -> TracePoints {
        let points = self.mutant_env.points.as_ref().lock().unwrap();
        points.clone()
    }

    pub fn compiler(&self) -> Compiler {
        self.compiler
    }
}

pub struct WasmerRuntimeFactory<'a> {
    compiled_code: Vec<u8>,
    discard_output: bool,
    map_dirs: &'a [(String, String)],
}

impl<'a> WasmerRuntimeFactory<'a> {
    pub fn new(
        module: &WasmModule,
        discard_output: bool,
        map_dirs: &'a [(String, String)],
    ) -> Result<Self> {
        let store = create_store(Compiler::Cranelift);
        let wasmer_module = create_module(module, &store)?;
        let compiled_code = wasmer_module.serialize()?;

        Ok(Self {
            compiled_code,
            discard_output,
            map_dirs,
        })
    }

    pub fn instantiate_mutant(&self, mutant_id: i64) -> Result<WasmerRuntime> {
        WasmerRuntime::new_from_cached_module(
            &self.compiled_code,
            self.discard_output,
            self.map_dirs,
            mutant_id,
            Compiler::Cranelift,
        )
    }
}

fn add_trace_function(store: &Store, import_object: &mut ImportObject, trace_env: &MutantEnv) {
    let mut exports = Exports::new();

    exports.insert(
        "__wasmut_trace",
        Function::new_native_with_env(store, trace_env.clone(), trace),
    );

    exports.insert(
        "__wasmut_check_mutant_id",
        Function::new_native_with_env(store, trace_env.clone(), check_mutant_id),
    );
    import_object.register("wasmut_api", exports);
}

fn create_store(compiler: Compiler) -> Store {
    // Define cost fuction for any executed instruction
    let cost_function = |_: &Operator| -> u64 { 1 };
    let metering = Arc::new(Metering::new(u64::MAX, cost_function));

    match compiler {
        Compiler::Singlepass => {
            let mut compiler_config = Singlepass::default();
            compiler_config.push_middleware(metering);
            Store::new(&Universal::new(compiler_config).engine())
        }
        Compiler::Cranelift => {
            let mut compiler_config = Cranelift::default();
            compiler_config.push_middleware(metering);
            Store::new(&Universal::new(compiler_config).engine())
        }
    }
}

fn create_module(module: &WasmModule, store: &Store) -> Result<Module> {
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
        let mut runtime = WasmerRuntime::new(&module, true, &[])?;

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
        let mut runtime = WasmerRuntime::new(&module, true, &[])?;

        let result = runtime.call_test_function(ExecutionPolicy::RunUntilLimit { limit: 1 })?;

        assert!(matches!(result, ExecutionResult::Timeout));

        Ok(())
    }

    #[test]
    fn compiler_display() {
        assert_eq!("Cranelift", format!("{}", Compiler::Cranelift));
        assert_eq!("Singlepass", format!("{}", Compiler::Singlepass));
    }

    #[test]
    fn test_correct_compiler() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let runtime = WasmerRuntime::new(&module, true, &[])?;

        assert!(matches!(runtime.compiler(), Compiler::Singlepass));

        let factory = WasmerRuntimeFactory::new(&module, true, &[])?;
        let runtime = factory.instantiate_mutant(0)?;

        assert!(matches!(runtime.compiler(), Compiler::Cranelift));
        Ok(())
    }
}
