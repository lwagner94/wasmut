use anyhow::Result;
use clap::{AppSettings, Parser, Subcommand};
use colored::*;
use env_logger::Builder;
use log::*;
use std::path::Path;

use wasmut::{
    addressresolver::AddressResolver, config::Config, executor::Executor, mutation::MutationEngine,
    policy::MutationPolicy, reporter, wasmmodule::WasmModule,
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

    /// Test stuff
    Test,
}

fn list_functions(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;
    let policy = MutationPolicy::from_config(config)?;

    for function in module.functions() {
        let check_result_str = if policy.check_function(&function) {
            "allowed: ".green()
        } else {
            "denied:  ".red()
        };
        println!("{check_result_str}{function}");
    }

    Ok(())
}

fn list_files(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;
    let policy = MutationPolicy::from_config(config)?;

    for file in module.source_files() {
        let check_result_str = if policy.check_file(&file) {
            "allowed: ".green()
        } else {
            "denied:  ".red()
        };
        println!("{check_result_str}{file}");
    }

    Ok(())
}

fn lookup(addr: u64, config: &Config) -> Result<()> {
    let bytes = std::fs::read(&config.module.wasmfile).unwrap();
    let resolver = AddressResolver::new(&bytes);

    let res = resolver.lookup_address(addr);

    dbg!(res);

    Ok(())
}

fn mutate(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    // dbg!(&mutations);

    let executor = Executor::new(config);
    let results = executor.execute(&module, &mutations)?;

    // dbg!(outcomes);

    let executed_mutants = reporter::prepare_results(&module, mutations, results);
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let cli_reporter = reporter::CLIReporter::new(&config.report, &mut lock)?;

    use reporter::Reporter;
    cli_reporter.report(&executed_mutants)?;

    reporter::generate_html(config, &executed_mutants)?;

    Ok(())
}

fn test(config: &Config) -> Result<()> {
    let module = WasmModule::from_file(&config.module.wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    dbg!(mutations.len());

    Ok(())
}

fn run_main() -> Result<()> {
    let cli = Cli::parse();

    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .format_target(false)
        .filter_module("wasmer_wasi", LevelFilter::Warn)
        .init();

    let config_path = cli
        .config
        .as_deref()
        .map_or(Path::new("./wasmut.toml"), Path::new);

    info!("Loading configuration file {config_path:?}");
    let config = Config::parse_file(config_path)?;
    let threads = config
        .engine
        .threads
        .unwrap_or_else(|| num_cpus::get() as u64);

    info!("Using {threads} workers to run mutants");
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads as usize)
        .build_global()
        .unwrap();

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
        Commands::Test => test(&config)?,
    }

    Ok(())
}

fn main() {
    match run_main() {
        Ok(_) => {}
        Err(e) => {
            error!("{e}");
            std::process::exit(1);
        }
    }
}
