use anyhow::Result;

use rayon::prelude::*;
use wasmut::runtime::*;
use wasmut::wasmmodule::WasmModule;

#[cfg(not(tarpaulin_include))]
fn main() -> Result<()> {
    use std::time;

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Insufficient args");
        std::process::exit(1);
    }

    // let bytecode = std::fs::read(&args[1])?;
    let module = WasmModule::from_file(&args[1])?;

    //discover_mutation_positions(&bytecode)?;

    let runtime_type = RuntimeType::Wasmtime;
    dbg!(&runtime_type);

    let mut runtime = create_runtime(runtime_type, module.clone())?;
    let tests = runtime.discover_test_functions()?;

    let mutations = module.discover_mutation_positions();
    // dbg!(&mutations);

    // let mut killed = 0;

    let start = time::Instant::now();
    let mut killed = 0;

    killed = mutations
        .par_iter()
        .fold(
            || 0,
            |mut killed, mutation| {
                let mut mutant = module.clone();
                mutant.mutate(mutation);
                let mut runtime = create_runtime(runtime_type, mutant).unwrap();

                for test in &tests {
                    match runtime
                        .call_test_function(
                            test,
                            wasmut::ExecutionPolicy::RunUntilLimit { limit: 100 },
                        )
                        .unwrap()
                    {
                        wasmut::ExecutionResult::Normal { return_value, .. } => {
                            if test.expected_result != (return_value != 0) {
                                killed += 1;
                                break;
                            }
                        }
                        wasmut::ExecutionResult::LimitExceeded => todo!(),
                        wasmut::ExecutionResult::Error => todo!(),
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
