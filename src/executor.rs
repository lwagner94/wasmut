use indicatif::{ParallelProgressIterator, ProgressBar};

use crate::mutation::Mutation;
use crate::policy::ExecutionPolicy;
use crate::runtime::ExecutionResult;
use crate::{config::Config, runtime, wasmmodule::WasmModule};
use anyhow::{bail, Result};

use rayon::prelude::*;

pub struct Executor {
    timeout_multiplier: f64,
}

impl Executor {
    pub fn new(config: &Config) -> Self {
        Executor {
            timeout_multiplier: config.engine().timeout_multiplier(),
        }
    }

    pub fn execute(&self, module: &WasmModule) -> Result<()> {
        // TODO: should the runtime own the module? If not, we can remove the clone here.
        let mut runtime = runtime::create_runtime(module.clone(), false)?;

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

    pub fn execute_mutants(
        &self,
        module: &WasmModule,
        mutations: &[Mutation],
    ) -> Result<Vec<ExecutionResult>> {
        let mut runtime = runtime::create_runtime(module.clone(), true)?;

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

                let mut runtime = runtime::create_runtime(module, true).unwrap();
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

    fn mutate_module(test_case: &str, mutations: &[Mutation]) -> Result<Vec<ExecutionResult>> {
        let module = WasmModule::from_file(&format!("testdata/{test_case}/test.wasm"))?;
        let executor = Executor::new(&Config::default());
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
