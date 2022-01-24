use crate::policy::ExecutionPolicy;
use crate::ExecutionResult;
use crate::{
    config::Config, error::Result, operator::Mutation, runtime, runtime::RuntimeType,
    wasmmodule::WasmModule,
};

use indicatif::{ParallelProgressIterator, ProgressBar};

use rayon::prelude::*;

pub struct Executor {}

impl Executor {
    pub fn new(_config: &Config) -> Self {
        Executor {}
    }

    pub fn execute(
        &self,
        module: &WasmModule,
        mutations: &[Mutation],
    ) -> Result<Vec<ExecutionOutcome>> {
        let pb = ProgressBar::new(mutations.len() as u64);

        let entry_point = {
            let mut runtime = runtime::create_runtime(RuntimeType::Wasmer, module.clone())?;
            runtime.discover_entry_point().unwrap()
        };

        let outcomes = mutations
            .par_iter()
            .progress_with(pb.clone())
            .map(|mutation| {
                // TODO: Remove mut by having clone_mutated() or something
                let mut module = module.clone();
                module.mutate(mutation);

                // TODO: configurable and adaptive
                let policy = ExecutionPolicy::RunUntilLimit { limit: 1000000 };

                let mut runtime = runtime::create_runtime(RuntimeType::Wasmer, module).unwrap();
                let result = runtime.call_test_function(&entry_point, policy).unwrap();

                match result {
                    ExecutionResult::FunctionReturn { return_value, .. } => {
                        if entry_point.expected_result != (return_value != 0) {
                            ExecutionOutcome::Killed
                        } else {
                            ExecutionOutcome::Alive
                        }
                    }
                    ExecutionResult::ProcessExit { exit_code, .. } => {
                        if entry_point.expected_result != (exit_code != 0) {
                            ExecutionOutcome::Killed
                        } else {
                            ExecutionOutcome::Alive
                        }
                    }
                    ExecutionResult::LimitExceeded => ExecutionOutcome::Timeout,
                    ExecutionResult::Error => ExecutionOutcome::ExecutionError,
                }
            })
            .collect();

        pb.finish();

        Ok(outcomes)
    }
}

// TODO: Come up with a better name once ExecutionResult in lib.rs is refactored
#[derive(Debug)]
pub enum ExecutionOutcome {
    Alive,
    Timeout,
    Killed,
    ExecutionError,
}
