pub mod addressresolver;
pub mod cliarguments;
pub mod config;
pub mod defaults;
pub mod executor;
pub mod mutation;
pub mod operator;
pub mod policy;
pub mod reporter;
pub mod runtime;
pub mod templates;
pub mod wasmmodule;

use anyhow::Result;

use colored::*;
use env_logger::Builder;
use log::*;
use std::path::Path;

use crate::cliarguments::{CLIArguments, CLICommand};

use crate::{
    addressresolver::AddressResolver, config::Config, executor::Executor, mutation::MutationEngine,
    policy::MutationPolicy, wasmmodule::WasmModule,
};

fn list_functions(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
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

fn list_files(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
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

fn lookup(wasmfile: &str, addr: u64) -> Result<()> {
    let bytes = std::fs::read(wasmfile).unwrap();
    let resolver = AddressResolver::new(&bytes);

    let res = resolver.lookup_address(addr);

    dbg!(res);

    Ok(())
}

fn mutate(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    dbg!(&mutations);

    let executor = Executor::new(config);
    let results = executor.execute(&module, &mutations)?;

    // dbg!(outcomes);

    let executed_mutants = reporter::prepare_results(&module, mutations, results);
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let cli_reporter = reporter::CLIReporter::new(config.report(), &mut lock)?;

    use reporter::Reporter;
    cli_reporter.report(&executed_mutants)?;

    reporter::generate_html(config, &executed_mutants)?;

    Ok(())
}

fn new_config(path: Option<String>) -> Result<()> {
    let path = path.unwrap_or_else(|| "wasmut.toml".into());
    Config::save_default_config(&path)?;
    info!("Created new configuration file {path}");
    Ok(())
}

fn test(_config: &Config) -> Result<()> {
    // let module = WasmModule::from_file(&config.module.wasmfile)?;
    // let mutator = MutationEngine::new(config)?;
    // let mutations = mutator.discover_mutation_positions(&module)?;

    // dbg!(mutations.len());

    Ok(())
}

fn load_config(config_path: Option<String>) -> Result<Config> {
    let config = if let Some(config_path) = config_path {
        // The user has supplied a configuration file
        info!("Loading user-specified configuration file {config_path:?}");
        Config::parse_file(config_path)?
    } else {
        let default_path = Path::new("wasmut.toml");

        if default_path.exists() {
            // wasmut.toml exists in current directory
            info!("Loading default configuration file {config_path:?}");
            Config::parse_file(default_path)?
        } else {
            // No config found, using defaults
            info!("No configuration file found or specified, using default config");
            Config::default()
        }
    };

    Ok(config)
}

fn init_rayon(config: &Config) {
    let threads = config.engine().threads();

    info!("Using {threads} workers");
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads as usize)
        .build_global()
        .unwrap();
}

pub fn run_main(cli: CLIArguments) -> Result<()> {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .format_target(false)
        .filter_module("wasmer_wasi", LevelFilter::Warn)
        .init();

    match cli.command {
        CLICommand::ListFunctions { config, wasmfile } => {
            let config = load_config(config)?;
            init_rayon(&config);
            list_functions(&wasmfile, &config)?;
        }
        CLICommand::ListFiles { config, wasmfile } => {
            let config = load_config(config)?;
            init_rayon(&config);
            list_files(&wasmfile, &config)?;
        }
        CLICommand::Mutate { config, wasmfile } => {
            let config = load_config(config)?;
            init_rayon(&config);
            mutate(&wasmfile, &config)?;
        }
        CLICommand::Lookup { wasmfile, address } => {
            lookup(&wasmfile, address)?;
        }
        CLICommand::NewConfig { path } => {
            new_config(path)?;
        }
        CLICommand::Test => {
            let config = load_config(None)?;
            init_rayon(&config);
            test(&config)?;
        }
    }

    Ok(())
}
