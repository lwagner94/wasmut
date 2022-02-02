use indicatif::{ParallelProgressIterator, ProgressBar};

use crate::error::Error;
use crate::mutation::Mutation;
use crate::policy::ExecutionPolicy;
use crate::{config::Config, error::Result, runtime, wasmmodule::WasmModule};
use crate::{defaults, runtime::ExecutionResult};

use rayon::prelude::*;

pub struct Executor {
    timeout_multiplier: f64,
}

impl Executor {
    pub fn new(config: &Config) -> Self {
        Executor {
            timeout_multiplier: config
                .engine
                .timeout_multiplier
                .unwrap_or(defaults::TIMEOUT_MULTIPLIER),
        }
    }

    pub fn execute(
        &self,
        module: &WasmModule,
        mutations: &[Mutation],
    ) -> Result<Vec<ExecutionResult>> {
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
            ExecutionResult::Timeout => {
                panic!("Execution limit exceeded even though we set no limit!")
            }
            ExecutionResult::Error => return Err(Error::WasmModuleFailed),
        };

        log::info!("Original module executed in {execution_cost} cycles");
        let limit = (execution_cost as f64 * self.timeout_multiplier).ceil() as u64;
        log::info!("Setting timeout to {limit} cycles");

        let hidden = false;
        let pb = if !hidden {
            ProgressBar::new(mutations.len() as u64)
        } else {
            ProgressBar::hidden()
        };

        let outcomes = mutations
            .par_iter()
            .progress_with(pb.clone())
            .map(|mutation| {
                // TODO: Remove mut by having clone_mutated() or something
                let mut module = module.clone();
                module.mutate(mutation);

                let policy = ExecutionPolicy::RunUntilLimit { limit };

                let mut runtime = runtime::create_runtime(module).unwrap();
                runtime.call_test_function(policy).unwrap()
            })
            .collect();

        pb.finish_and_clear();

        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn execute_module(test_case: &str) -> Result<Vec<ExecutionResult>> {
        let module = WasmModule::from_file(&format!("testdata/{test_case}/test.wasm"))?;
        let executor = Executor::new(&Config::default());
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
}
