use indicatif::{ParallelProgressIterator, ProgressBar};

use crate::mutation::MutationLocation;
use crate::operator::InstructionReplacement;
use crate::policy::ExecutionPolicy;
use crate::runtime::wasmer::{WasmerRuntime, WasmerRuntimeFactory};
use crate::runtime::{ExecutionResult, TracePoints};
use crate::{config::Config, wasmmodule::WasmModule};
use anyhow::{bail, Result};

use rayon::prelude::*;

#[derive(Debug)]
pub struct ExecutedMutant {
    pub offset: u64,
    pub outcome: ExecutionResult,
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

    /// If true, only a single mutant containing all possible mutations
    /// will be generated, reducing compilation time.
    meta_mutant: bool,
}

impl<'a> Executor<'a> {
    /// Create `Executor` based on wasmut configuration
    pub fn new(config: &'a Config) -> Self {
        Executor {
            timeout_multiplier: config.engine().timeout_multiplier(),
            mapped_dirs: config.engine().map_dirs(),
            coverage: config.engine().coverage_based_execution(),
            meta_mutant: config.engine().meta_mutant(),
        }
    }

    /// Execute a WebAssembly module, without performing any mutations.
    ///
    /// The stdout/stderr output of the module will not be supressed
    pub fn execute(&self, module: &WasmModule) -> Result<()> {
        let mut runtime = WasmerRuntime::new(module, false, self.mapped_dirs)?;

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

    pub fn execute_mutants(
        &self,
        module: &WasmModule,
        locations: &[MutationLocation],
    ) -> Result<Vec<ExecutedMutant>> {
        let trace_points = if self.coverage {
            self.get_trace_points(module)?
        } else {
            TracePoints::default()
        };

        let outcomes = if self.meta_mutant {
            self.execute_mutants_meta(module, locations, trace_points)
        } else {
            self.execute_mutants_one_by_one(module, locations, trace_points)
        }?;

        if self.coverage {
            let skipped = outcomes.iter().fold(0, |acc, current| match current {
                ExecutedMutant {
                    outcome: ExecutionResult::Skipped,
                    ..
                } => acc + 1,
                _ => acc,
            });

            log::info!(
                "Skipped {}/{} mutant because of missing code coverage",
                skipped,
                outcomes.len()
            );
        }

        Ok(outcomes)
    }

    /// Execute mutants and gather results
    ///
    /// During execution, stdout and stderr are supressed
    fn execute_mutants_one_by_one(
        &self,
        module: &WasmModule,
        locations: &[MutationLocation],
        trace_points: TracePoints,
    ) -> Result<Vec<ExecutedMutant>> {
        let mut runtime = WasmerRuntime::new(module, true, self.mapped_dirs)?;

        let limit = self.calculate_execution_cost(&mut runtime)?;

        let pb = ProgressBar::new(locations.len() as u64);

        let outcomes: Vec<ExecutedMutant> = locations
            .par_iter()
            .progress_with(pb.clone())
            .flat_map(|location| {
                location
                    .mutations
                    .iter()
                    .enumerate()
                    .map(|(cnt, mutation)| {
                        if self.coverage && !trace_points.is_covered(location.offset) {
                            return ExecutedMutant {
                                offset: location.offset,
                                outcome: ExecutionResult::Skipped,
                                operator: mutation.operator.clone(),
                            };
                        }

                        let module = module.clone_and_mutate(location, cnt);

                        let mut runtime = WasmerRuntime::new(&module, true, self.mapped_dirs)
                            .expect("Failed to create runtime");

                        let policy = ExecutionPolicy::RunUntilLimit { limit };
                        let result = runtime
                            .call_test_function(policy)
                            .expect("Failed to execute module after applying mutation");

                        ExecutedMutant {
                            offset: location.offset,
                            outcome: result,
                            operator: mutation.operator.clone(),
                        }
                    })
                    .collect::<Vec<ExecutedMutant>>()
            })
            .collect();

        pb.finish_and_clear();

        Ok(outcomes)
    }

    fn execute_mutants_meta(
        &self,
        module: &WasmModule,
        locations: &[MutationLocation],
        trace_points: TracePoints,
    ) -> Result<Vec<ExecutedMutant>> {
        let meta_mutant = module.clone_and_mutate_all(locations)?;
        let factory = WasmerRuntimeFactory::new(&meta_mutant, true, self.mapped_dirs)?;

        let mut runtime = factory.instantiate_mutant(0).unwrap();
        let limit = self.calculate_execution_cost(&mut runtime)?;

        let pb = ProgressBar::new(locations.len() as u64);

        let outcomes: Vec<ExecutedMutant> = locations
            .par_iter()
            .progress_with(pb.clone())
            .flat_map(|location| {
                location
                    .mutations
                    .iter()
                    .map(|mutation| {
                        if self.coverage && !trace_points.is_covered(location.offset) {
                            return ExecutedMutant {
                                offset: location.offset,
                                outcome: ExecutionResult::Skipped,
                                operator: mutation.operator.clone(),
                            };
                        }

                        let policy = ExecutionPolicy::RunUntilLimit { limit };
                        let mut runtime = factory
                            .instantiate_mutant(mutation.id)
                            .expect("Failed to create runtime");
                        let result = runtime
                            .call_test_function(policy)
                            .expect("Failed to execute module after applying mutation");

                        ExecutedMutant {
                            offset: location.offset,
                            outcome: result,
                            operator: mutation.operator.clone(),
                        }
                    })
                    .collect::<Vec<ExecutedMutant>>()
            })
            .collect();

        pb.finish_and_clear();

        Ok(outcomes)
    }

    fn calculate_execution_cost(&self, runtime: &mut WasmerRuntime) -> Result<u64> {
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
            ExecutionResult::Skipped => panic!("Runtime returned ExecutionResult::Skipped"),
        };
        log::info!("Original module executed in {execution_cost} cycles");
        let limit = (execution_cost as f64 * self.timeout_multiplier).ceil() as u64;
        log::info!("Setting timeout to {limit} cycles");
        Ok(limit)
    }

    fn get_trace_points(&self, module: &WasmModule) -> Result<TracePoints> {
        let mut module = module.clone();
        module.insert_trace_points()?;
        let mut runtime = WasmerRuntime::new(&module, true, self.mapped_dirs)?;

        let trace_points = match runtime.call_test_function(ExecutionPolicy::RunUntilReturn)? {
            ExecutionResult::ProcessExit { exit_code, .. } => {
                if exit_code != 0 {
                    bail!("Module without any mutations returned exit code {exit_code}");
                }
                runtime.trace_points()
            }
            ExecutionResult::Timeout => {
                panic!("Execution limit exceeded even though we set no limit!")
            }
            ExecutionResult::Error => bail!("Module failed to execute"),
            ExecutionResult::Skipped => panic!("Runtime returned ExecutionResult::Skipped"),
        };
        Ok(trace_points)
    }
}

#[cfg(test)]
mod tests {

    use parity_wasm::elements::Instruction;

    use crate::{
        mutation::Mutation,
        operator::ops::{
            BinaryOperatorAddToSub, ConstReplaceNonZero, RelationalOperatorLtToGe,
            RelationalOperatorLtToLe,
        },
    };

    use super::*;

    fn mutate_module(
        test_case: &str,
        mutations: &[MutationLocation],
    ) -> Result<Vec<ExecutedMutant>> {
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
            id: 1,
            operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
        }];

        let location = MutationLocation {
            function_number: 1,
            statement_number: 2,
            offset: 34,
            mutations,
        };

        let result = mutate_module("simple_add", &[location])?;
        assert!(matches!(
            result[0],
            ExecutedMutant {
                outcome: ExecutionResult::ProcessExit { exit_code: 1, .. },
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
            ExecutedMutant {
                outcome: ExecutionResult::Skipped,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn meta_results_should_be_equal() {
        let locations = [
            MutationLocation {
                function_number: 1,
                statement_number: 5,
                offset: 42,
                mutations: vec![
                    Mutation {
                        id: 3,
                        operator: Box::new(
                            RelationalOperatorLtToGe::new(&Instruction::I32LtS).unwrap(),
                        ),
                    },
                    Mutation {
                        id: 4,
                        operator: Box::new(
                            RelationalOperatorLtToLe::new(&Instruction::I32LtS).unwrap(),
                        ),
                    },
                ],
            },
            MutationLocation {
                function_number: 1,
                statement_number: 20,
                offset: 69,
                mutations: vec![Mutation {
                    id: 12,
                    operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
                }],
            },
        ];

        let module = WasmModule::from_file("testdata/factorial/test.wasm").unwrap();
        let config = Config::parse(
            r#"
            [engine]
            coverage_based_execution = false
            meta_mutant = false
        "#,
        )
        .unwrap();
        let executor = Executor::new(&config);
        let no_meta_results = executor.execute_mutants(&module, &locations).unwrap();

        let config = Config::parse(
            r#"
            [engine]
            coverage_based_execution = false
            meta_mutant = true
        "#,
        )
        .unwrap();
        let executor = Executor::new(&config);
        let meta_results = executor.execute_mutants(&module, &locations).unwrap();

        assert_eq!(no_meta_results.len(), meta_results.len());

        impl PartialEq for ExecutionResult {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (
                        Self::ProcessExit {
                            exit_code: l_exit_code,
                            ..
                        },
                        Self::ProcessExit {
                            exit_code: r_exit_code,
                            ..
                        },
                    ) => l_exit_code == r_exit_code,
                    _ => core::mem::discriminant(self) == core::mem::discriminant(other),
                }
            }
        }

        for (a, b) in no_meta_results.iter().zip(&meta_results) {
            assert_eq!(a.outcome, b.outcome);
        }
    }
}
