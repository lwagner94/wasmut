use anyhow::Result;
use clap::{App, Arg, SubCommand, ArgMatches, AppSettings};

use rayon::prelude::*;
use wasmut::{
    policy::{ExecutionPolicy, MutationPolicyBuilder},
    runtime::{create_runtime, RuntimeType},
    wasmmodule::WasmModule,
    ExecutionResult,
};

fn build_argparser() -> ArgMatches<'static> {
    App::new("wasmut")
        .version("0.0.1")
        .author("Lukas Wagner <lwagner94@posteo.at>")
        .about("Mutation testing for WebAssembly")
        .setting(AppSettings::ColorAlways)
        .subcommand(
            SubCommand::with_name("list-functions").arg(
                Arg::with_name("INPUT")
                    .help("Sets the input file to use")
                    .required(true),
            ),
        )
        .subcommand(
            SubCommand::with_name("mutate").arg(
                Arg::with_name("INPUT")
                    .help("Sets the input file to use")
                    .required(true),
            ),
        )
        .get_matches()
}

fn list_functions(wasmfile: &str) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    for function in module.functions() {
        println!("{}", function);
    }

    Ok(())
}

fn mutate(wasmfile: &str) -> Result<()> {
   use std::time;


    let module = WasmModule::from_file(wasmfile)?;

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

fn main() -> Result<()> {
    let cli = build_argparser();

    if let Some(cli) = cli.subcommand_matches("list-functions") {
        let input_file = cli.value_of("INPUT").unwrap();

        list_functions(input_file)?;
    }

    if let Some(cli) = cli.subcommand_matches("mutate") {
        let input_file = cli.value_of("INPUT").unwrap();

        mutate(input_file)?;
    }

    Ok(())

 
}
