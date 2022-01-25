use std::sync::{Arc, Mutex};

use crate::error::Error;
use crate::policy::ExecutionPolicy;
use crate::{config::Config, error::Result, operator::Mutation, runtime, wasmmodule::WasmModule};
use crate::{defaults, ExecutionResult};

use rayon::prelude::*;

type ProgressCallback = Box<dyn Fn(ExecutionOutcome) + Send>;

pub struct Executor {
    timeout_multiplier: f64,
    progress_callback: Option<Arc<Mutex<ProgressCallback>>>,
}

impl Executor {
    pub fn new(config: &Config, callback: Option<ProgressCallback>) -> Self {
        let cb = callback.map(|cb| Arc::new(Mutex::new(cb)));
        Executor {
            timeout_multiplier: config
                .engine
                .timeout_multiplier
                .unwrap_or(defaults::TIMEOUT_MULTIPLIER),
            progress_callback: cb,
        }
    }

    pub fn execute(
        &self,
        module: &WasmModule,
        mutations: &[Mutation],
    ) -> Result<Vec<ExecutionOutcome>> {
        let mut runtime = runtime::create_runtime(module.clone())?;

        let execution_cost = match runtime.call_test_function(ExecutionPolicy::RunUntilReturn)? {
            ExecutionResult::ProcessExit {
                exit_code,
                execution_cost,
            } => {
                if exit_code == 0 {
                    execution_cost
                } else {
                    return Err(Error::WasmModuleNonzeroExit(exit_code));
                }
            }
            ExecutionResult::LimitExceeded => {
                panic!("Execution limit exceeded even though we set no limit!")
            }
            ExecutionResult::Error => return Err(Error::WasmModuleFailed),
        };

        log::info!("Original module executed in {execution_cost} cycles");
        let limit = (execution_cost as f64 * self.timeout_multiplier).ceil() as u64;
        log::info!("Setting timeout to {limit} cycles");

        let outcomes = mutations
            .par_iter()
            .map_with(&self.progress_callback, |progress_callback, mutation| {
                // TODO: Remove mut by having clone_mutated() or something
                let mut module = module.clone();
                module.mutate(mutation);

                let policy = ExecutionPolicy::RunUntilLimit { limit };

                let mut runtime = runtime::create_runtime(module).unwrap();
                let result = runtime.call_test_function(policy).unwrap();

                let outcome = match result {
                    ExecutionResult::ProcessExit { exit_code, .. } => {
                        if exit_code == 0 {
                            ExecutionOutcome::Alive
                        } else {
                            ExecutionOutcome::Killed
                        }
                    }
                    ExecutionResult::LimitExceeded => ExecutionOutcome::Timeout,
                    ExecutionResult::Error => ExecutionOutcome::ExecutionError,
                };

                if let Some(progress_callback) = progress_callback {
                    progress_callback.lock().unwrap()(outcome.clone());
                }

                outcome
            })
            .collect();

        Ok(outcomes)
    }
}

// TODO: Come up with a better name once ExecutionResult in lib.rs is refactored
#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    Alive,
    Timeout,
    Killed,
    ExecutionError,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use crate::mutation::MutationEngine;

    use super::*;

    fn execute_module(test_case: &str) -> Result<Vec<ExecutionOutcome>> {
        let module = WasmModule::from_file(&format!("testdata/{test_case}/test.wasm"))?;
        let executor = Executor::new(&Config::default(), None);
        executor.execute(&module, &[])
    }

    #[test]
    fn original_module_nonzero_exit() -> Result<()> {
        let result = execute_module("nonzero_exit");
        assert!(matches!(result, Err(Error::WasmModuleNonzeroExit(1))));
        Ok(())
    }

    #[test]
    fn original_module_rust_fail() -> Result<()> {
        let result = execute_module("rust_fail");
        assert!(matches!(result, Err(Error::WasmModuleFailed)));
        Ok(())
    }

    #[test]
    fn no_mutants() -> Result<()> {
        let result = execute_module("simple_add");
        assert!(matches!(result, Ok(..)));
        Ok(())
    }

    #[test]
    fn callback() -> Result<()> {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let cb = move |_| called_clone.store(true, std::sync::atomic::Ordering::Relaxed);

        let mut config = Config::parse_file("testdata/simple_add/wasmut.toml")?;
        config.engine.threads = Some(1);
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let mutator = MutationEngine::new(&config)?;

        let mutations = mutator.discover_mutation_positions(&module);

        let executor = Executor::new(&config, Some(Box::new(cb)));
        let result = executor.execute(&module, &mutations);
        assert!(matches!(result, Ok(..)));
        assert!(called.load(std::sync::atomic::Ordering::Relaxed));
        Ok(())
    }
}
