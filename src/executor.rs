use indicatif::{ParallelProgressIterator, ProgressBar};

use crate::mutation::Mutation;
use crate::policy::ExecutionPolicy;
use crate::runtime::ExecutionResult;
use crate::{config::Config, runtime, wasmmodule::WasmModule};
use anyhow::{bail, Result};

use rayon::prelude::*;

/// Execution engine for WebAssembly modules
pub struct Executor<'a> {
    /// Timeout multiplier used when executing mutants
    /// Timeout in cycles is calculated by multiplying
    /// this factor with the measured number of cycles
    timeout_multiplier: f64,

    /// List of directory mappings
    mapped_dirs: &'a [(String, String)],
}

impl<'a> Executor<'a> {
    /// Create `Executor` based on wasmut configuration
    pub fn new(config: &'a Config) -> Self {
        Executor {
            timeout_multiplier: config.engine().timeout_multiplier(),
            mapped_dirs: config.engine().map_dirs(),
        }
    }

    /// Execute a WebAssembly module, without performing any mutations.
    ///
    /// The stdout/stderr output of the module will not be supressed
    pub fn execute(&self, module: &WasmModule) -> Result<()> {
        let mut runtime = runtime::create_runtime(module, false, self.mapped_dirs)?;

        // TODO: Code duplication?
        match runtime.call_test_function(ExecutionPolicy::RunUntilReturn)? {
            ExecutionResult::ProcessExit {
                exit_code,
                execution_cost,
            } => {
                if exit_code == 0 {
                    log::info!("Module executed in {execution_cost} cycles");
                } else {
                    bail!("Module returned exit code {exit_code}");
                }
            }
            ExecutionResult::Timeout => {
                panic!("Execution limit exceeded even though we set no limit!")
            }
            ExecutionResult::Error => bail!("Module failed to execute"),
        };

        Ok(())
    }

    /// Execute mutants and gather results
    ///
    /// During execution, stdout and stderr are supressed
    pub fn execute_mutants(
        &self,
        module: &WasmModule,
        mutations: &[Mutation],
    ) -> Result<Vec<ExecutionResult>> {
        let mut runtime = runtime::create_runtime(module, true, self.mapped_dirs)?;

        let execution_cost = match runtime.call_test_function(ExecutionPolicy::RunUntilReturn)? {
            ExecutionResult::ProcessExit {
                exit_code,
                execution_cost,
            } => {
                if exit_code == 0 {
                    execution_cost
                } else {
                    bail!("Module without any mutations returned exit code {exit_code}");
                }
            }
            ExecutionResult::Timeout => {
                panic!("Execution limit exceeded even though we set no limit!")
            }
            ExecutionResult::Error => bail!("Module failed to execute"),
        };

        log::info!("Original module executed in {execution_cost} cycles");
        let limit = (execution_cost as f64 * self.timeout_multiplier).ceil() as u64;
        log::info!("Setting timeout to {limit} cycles");

        let pb = ProgressBar::new(mutations.len() as u64);

        let outcomes = mutations
            .par_iter()
            .progress_with(pb.clone())
            .map(|mutation| {
                let module = module.mutated_clone(mutation);

                let policy = ExecutionPolicy::RunUntilLimit { limit };

                let mut runtime = runtime::create_runtime(&module, true, self.mapped_dirs).unwrap();
                runtime.call_test_function(policy).unwrap()
            })
            .collect();

        pb.finish_and_clear();

        Ok(outcomes)
    }

    pub fn execute_coverage(&self, module: &WasmModule) -> Result<()> {
        let mut runtime = runtime::create_runtime(&module.clone(), true, &[])?;

        let execution_cost = match runtime.call_test_function(ExecutionPolicy::RunUntilReturn)? {
            ExecutionResult::ProcessExit {
                exit_code,
                execution_cost,
            } => {
                if exit_code == 0 {
                    execution_cost
                } else {
                    bail!("Module without any mutations returned exit code {exit_code}");
                }
            }
            ExecutionResult::Timeout => {
                panic!("Execution limit exceeded even though we set no limit!")
            }
            ExecutionResult::Error => bail!("Module failed to execute"),
        };

        log::info!("Coverage executed in {execution_cost} cycles");

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn mutate_module(test_case: &str, mutations: &[Mutation]) -> Result<Vec<ExecutionResult>> {
        let path = format!("testdata/{test_case}/test.wasm");
        let module = WasmModule::from_file(&path)?;
        let config = Config::default();
        let executor = Executor::new(&config);
        executor.execute_mutants(&module, mutations)
    }

    #[test]
    fn original_module_nonzero_exit() -> Result<()> {
        let result = mutate_module("nonzero_exit", &[]);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn original_module_rust_fail() -> Result<()> {
        let result = mutate_module("rust_fail", &[]);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn no_mutants() -> Result<()> {
        let result = mutate_module("simple_add", &[]);
        assert!(matches!(result, Ok(..)));
        Ok(())
    }

    #[test]
    fn execute_mutant() -> Result<()> {
        let mutations = vec![Mutation {
            function_number: 1,
            statement_number: 2,
            offset: 34,
            operator: Box::new(crate::operator::ops::BinaryOperatorAddToSub(
                parity_wasm::elements::Instruction::I32Add,
                parity_wasm::elements::Instruction::I32Sub,
            )),
        }];

        let result = mutate_module("simple_add", &mutations)?;
        assert!(matches!(
            result[0],
            ExecutionResult::ProcessExit { exit_code: 1, .. }
        ));

        Ok(())
    }
}
