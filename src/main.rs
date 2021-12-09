use anyhow::Result;

use rayon::prelude::*;
use wasmut::{
    policy::{ExecutionPolicy, MutationPolicyBuilder},
    runtime::{create_runtime, RuntimeType},
    wasmmodule::WasmModule,
    ExecutionResult,
};

fn main() -> Result<()> {
    use std::time;

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Insufficient args");
        std::process::exit(1);
    }

    let module = WasmModule::from_file(&args[1])?;

    let runtime_type = RuntimeType::Wasmtime;
    dbg!(&runtime_type);

    let mut runtime = create_runtime(runtime_type, module.clone())?;
    let entry_point = runtime.discover_entry_point().unwrap();
    let tests = vec![entry_point];

    let mutation_policy = MutationPolicyBuilder::new()
        .allow_function("^add")
        .build()?;

    let mutations = module.discover_mutation_positions(&mutation_policy);

    let start = time::Instant::now();

    let killed: u32 = mutations
        .par_iter()
        .fold(
            || 0,
            |mut killed, mutation| {
                let mut mutant = module.clone();
                mutant.mutate(mutation);
                let mut runtime = create_runtime(runtime_type, mutant).unwrap();

                for test in &tests {
                    match runtime
                        .call_test_function(test, ExecutionPolicy::RunUntilLimit { limit: 100 })
                        .unwrap()
                    {
                        ExecutionResult::FunctionReturn { return_value, .. } => {
                            if test.expected_result != (return_value != 0) {
                                killed += 1;
                                break;
                            }
                        }
                        ExecutionResult::ProcessExit { exit_code, .. } => {
                            if test.expected_result != (exit_code != 0) {
                                killed += 1;
                                break;
                            }
                        }
                        ExecutionResult::LimitExceeded => todo!(),
                        ExecutionResult::Error => todo!(),
                    }
                }

                killed
            },
        )
        .sum();

    print!("Killed {}/{} mutants ", killed, mutations.len());
    println!("in {}s.", start.elapsed().as_millis() as f64 / 1000.0);

    Ok(())
}
