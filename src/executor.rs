use indicatif::{ParallelProgressIterator, ProgressBar};

use crate::mutation::{Mutation, MutationLocation};
use crate::operator::InstructionReplacement;
use crate::policy::ExecutionPolicy;
use crate::reporter::{ExecutedMutant, MutationOutcome};
use crate::runtime::ExecutionResult;
use crate::{config::Config, runtime, wasmmodule::WasmModule};
use anyhow::{bail, Result};

use rayon::prelude::*;

#[derive(Debug)]
pub struct ExecutedMutantFromEngine {
    pub offset: u64,
    pub outcome: MutationOutcome,
    pub operator: Box<dyn InstructionReplacement>,
}

/// Execution engine for WebAssembly modules
pub struct Executor<'a> {
    /// Timeout multiplier used when executing mutants
    /// Timeout in cycles is calculated by multiplying
    /// this factor with the measured number of cycles
    timeout_multiplier: f64,

    /// List of directory mappings
    mapped_dirs: &'a [(String, String)],

    /// If set to true, mutants that have no chance of being ever executed
    /// will be skipped.
    coverage: bool,
}

impl<'a> Executor<'a> {
    /// Create `Executor` based on wasmut configuration
    pub fn new(config: &'a Config) -> Self {
        Executor {
            timeout_multiplier: config.engine().timeout_multiplier(),
            mapped_dirs: config.engine().map_dirs(),
            coverage: config.engine().coverage_based_execution(),
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
            ExecutionResult::Skipped => panic!("Runtime returned ExecutionResult::Skipped"),
        };

        Ok(())
    }

    /// Execute mutants and gather results
    ///
    /// During execution, stdout and stderr are supressed
    pub fn execute_mutants(
        &self,
        module: &WasmModule,
        mutations: &[MutationLocation],
    ) -> Result<Vec<ExecutedMutantFromEngine>> {
        let mut runtime = if self.coverage {
            let mut module = module.clone();
            module.insert_trace_points()?;
            runtime::create_runtime(&module, true, self.mapped_dirs)?
        } else {
            runtime::create_runtime(module, true, self.mapped_dirs)?
        };

        let mut execution_cost =
            match runtime.call_test_function(ExecutionPolicy::RunUntilReturn)? {
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
                ExecutionResult::Skipped => panic!("Runtime returned ExecutionResult::Skipped"),
            };

        if self.coverage {
            // For every instruction, a I32Const and a Call instruction will be inserted
            execution_cost /= 3;
        }

        log::info!("Original module executed in {execution_cost} cycles");
        let limit = (execution_cost as f64 * self.timeout_multiplier).ceil() as u64;
        log::info!("Setting timeout to {limit} cycles");

        let pb = ProgressBar::new(mutations.len() as u64);

        let trace_points = runtime.trace_points();

        let outcomes: Vec<ExecutedMutantFromEngine> = mutations
            .par_iter()
            .progress_with(pb.clone())
            .flat_map(|location| {
                // for mutation in location.mutations {

                // }
                let offset = location.offset;

                location
                    .mutations
                    .iter()
                    .enumerate()
                    .map(|(cnt, mutation)| {
                        if self.coverage && !trace_points.contains(&offset) {
                            return ExecutedMutantFromEngine {
                                // offset: location.offset,
                                offset,
                                outcome: MutationOutcome::Alive, // TODO: Use own outcome variant for skipped?
                                operator: mutation.operator.clone(),
                            };
                        }

                        let module = module.clone_and_mutate(location, cnt);

                        let policy = ExecutionPolicy::RunUntilLimit { limit };

                        let mut runtime =
                            runtime::create_runtime(&module, true, self.mapped_dirs).unwrap();
                        let result = runtime.call_test_function(policy).unwrap();

                        ExecutedMutantFromEngine {
                            // offset: location.offset,
                            offset,
                            outcome: result.into(),
                            operator: mutation.operator.clone(),
                        }
                    })
                    .collect::<Vec<ExecutedMutantFromEngine>>()
            })
            .collect();

        pb.finish_and_clear();

        // if self.coverage {
        //     let skipped = outcomes.iter().fold(0, |acc, current| match current {
        //         ExecutionResult::Skipped => acc + 1,
        //         _ => acc,
        //     });

        //     log::info!(
        //         "Skipped {}/{} mutant because of missing code coverage",
        //         skipped,
        //         outcomes.len()
        //     );
        // }

        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {

    use parity_wasm::elements::Instruction;

    use crate::operator::ops::{BinaryOperatorAddToSub, ConstReplaceNonZero};

    use super::*;

    fn mutate_module(
        test_case: &str,
        mutations: &[MutationLocation],
    ) -> Result<Vec<ExecutedMutantFromEngine>> {
        let path = format!("testdata/{test_case}/test.wasm");
        let module = WasmModule::from_file(&path)?;
        let config = Config::parse(
            r#"
            [engine]
            coverage_based_execution = false
        "#,
        )?;
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
            id: 0,
            operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
        }];

        let location = MutationLocation {
            function_number: 1,
            statement_number: 2,
            offset: 37,
            mutations,
        };

        let result = mutate_module("simple_add", &[location])?;
        assert!(matches!(
            result[0],
            ExecutedMutantFromEngine {
                outcome: MutationOutcome::Killed,
                ..
            }
        ));

        Ok(())
    }

    #[test]
    fn skip_because_not_covered() -> Result<()> {
        let mutations = vec![Mutation {
            id: 0,
            operator: Box::new(ConstReplaceNonZero::new(&Instruction::I32Const(-1)).unwrap()),
        }];

        let location = MutationLocation {
            function_number: 3,
            statement_number: 0,
            offset: 46,
            mutations,
        };

        let module = WasmModule::from_file("testdata/no_coverage/test.wasm")?;
        let config = Config::parse(
            r#"
            [engine]
            coverage_based_execution = true
        "#,
        )?;
        let executor = Executor::new(&config);
        let result = executor.execute_mutants(&module, &[location])?;

        assert!(matches!(
            result[0],
            ExecutedMutantFromEngine {
                outcome: MutationOutcome::Alive,
                ..
            }
        ));
        Ok(())
    }
}
