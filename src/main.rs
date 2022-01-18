use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use std::time;

use rayon::prelude::*;
use wasmut::{
    addressresolver::AddressResolver,
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
        .subcommand(
            SubCommand::with_name("lookup")
                .arg(
                    Arg::with_name("INPUT")
                        .help("Sets the input file to use")
                        .required(true),
                )
                .arg(
                    Arg::with_name("ADDR")
                        .help("Address to look up")
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

fn lookup(wasmfile: &str, addr: u64) -> Result<()> {
    let bytes = std::fs::read(wasmfile).unwrap();
    let resolver = AddressResolver::new(&bytes);

    let res = resolver.lookup_address(addr)?;

    dbg!(res);

    Ok(())
}

fn mutate(wasmfile: &str) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;

    let runtime_type = RuntimeType::Wasmer;
    dbg!(&runtime_type);
    let start = time::Instant::now();
    let mut runtime = create_runtime(runtime_type, module.clone())?;
    println!(
        "Created runtime in {}s.",
        start.elapsed().as_millis() as f64 / 1000.0
    );
    let entry_point = runtime.discover_entry_point().unwrap();
    let tests = vec![entry_point];

    let mutation_policy = MutationPolicyBuilder::new().allow_function("").build()?;

    let mutations = module.discover_mutation_positions(&mutation_policy);

    //dbg!(&mutations);
    dbg!(&mutations.len());

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
                        .call_test_function(
                            test,
                            ExecutionPolicy::RunUntilLimit { limit: 10000000 },
                        )
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
                        ExecutionResult::LimitExceeded => {
                            killed += 1;
                            println!("timeout");
                            break;
                        }
                        ExecutionResult::Error => {
                            killed += 1;
                            println!("Error.");
                            break;
                        }
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

    if let Some(cli) = cli.subcommand_matches("lookup") {
        let input_file = cli.value_of("INPUT").unwrap();
        let addr = cli.value_of("ADDR").unwrap().parse::<u64>().unwrap();

        lookup(input_file, addr)?;
    }

    Ok(())
}
