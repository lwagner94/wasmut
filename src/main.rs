use anyhow::Result;
use clap::{AppSettings, Parser, Subcommand};
use std::{path::Path, time};

use rayon::prelude::*;
use wasmut::{
    addressresolver::AddressResolver,
    config::Config,
    policy::{ExecutionPolicy, MutationPolicy},
    runtime::{create_runtime, RuntimeType},
    wasmmodule::WasmModule,
    ExecutionResult,
};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
struct Cli {
    /// Path to wasmut.toml configuration
    #[clap(short, long)]
    config: Option<String>,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all functions of the binary
    ListFunctions {},
    /// List all files
    ListFiles {},
    /// Run mutants
    Mutate {},
    /// Lookup an address
    Lookup { address: u64 },
}

fn list_functions(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;
    let policy = MutationPolicy::from_config(config)?;

    for function in module.functions() {
        if policy.check_function(&function) {
            colour::dark_green!("allowed: ")
        } else {
            colour::dark_red!("denied:  ")
        }
        println!("{function}");
    }

    Ok(())
}

fn list_files(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;
    let policy = MutationPolicy::from_config(config)?;

    for file in module.source_files() {
        if policy.check_file(&file) {
            colour::dark_green!("allowed: ")
        } else {
            colour::dark_red!("denied:  ")
        }
        println!("{file}");
    }

    Ok(())
}

fn lookup(addr: u64, config: &Config) -> Result<()> {
    let bytes = std::fs::read(&config.module.wasmfile).unwrap();
    let resolver = AddressResolver::new(&bytes);

    let res = resolver.lookup_address(addr)?;

    dbg!(res);

    Ok(())
}

fn mutate(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;

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

    let mutation_policy = MutationPolicy::from_config(config)?;

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
                                println!("K");
                                killed += 1;
                                break;
                            } else {
                                println!("A");
                            }
                        }
                        ExecutionResult::ProcessExit { exit_code, .. } => {
                            if test.expected_result != (exit_code != 0) {
                                println!("K");
                                killed += 1;
                                break;
                            } else {
                                println!("A");
                            }
                        }
                        ExecutionResult::LimitExceeded => {
                            killed += 1;
                            println!("T");
                            break;
                        }
                        ExecutionResult::Error => {
                            killed += 1;
                            println!("E");
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

use env_logger::Builder;
use log::*;

fn main() -> Result<()> {
    let cli = Cli::parse();

    Builder::new().filter_level(LevelFilter::max()).init();

    let config_path = cli
        .config
        .as_deref()
        .map_or(Path::new("./wasmut.toml"), Path::new);

    let config = Config::parse_file(config_path)?;

    match cli.command {
        Commands::ListFunctions {} => {
            list_functions(&config)?;
        }
        Commands::ListFiles {} => {
            list_files(&config)?;
        }
        Commands::Mutate {} => {
            mutate(&config)?;
        }
        Commands::Lookup { address } => {
            lookup(address, &config)?;
        }
    }

    Ok(())
}
