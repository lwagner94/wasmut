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
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all functions of the binary
    ListFunctions {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,
        wasmfile: String,
    },
    /// List all files
    ListFiles {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,
        wasmfile: String,
    },
    /// Run mutants
    Mutate {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,
        wasmfile: String,
    },
    /// Lookup an address
    Lookup {
        wasmfile: String,
        address: u64,
    },

    NewConfig {
        path: Option<String>,
    },

    /// Test stuff
    Test,
}

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
    Config::save_default_config(path)?;
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

    info!("Using {threads} workers to run mutants");
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads as usize)
        .build_global()
        .unwrap();
}

fn run_main() -> Result<()> {
    let cli = Cli::parse();

    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .format_target(false)
        .filter_module("wasmer_wasi", LevelFilter::Warn)
        .init();

    match cli.command {
        Commands::ListFunctions { config, wasmfile } => {
            let config = load_config(config)?;
            init_rayon(&config);
            list_functions(&wasmfile, &config)?;
        }
        Commands::ListFiles { config, wasmfile } => {
            let config = load_config(config)?;
            init_rayon(&config);
            list_files(&wasmfile, &config)?;
        }
        Commands::Mutate { config, wasmfile } => {
            let config = load_config(config)?;
            init_rayon(&config);
            mutate(&wasmfile, &config)?;
        }
        Commands::Lookup { wasmfile, address } => {
            lookup(&wasmfile, address)?;
        }
        Commands::NewConfig { path } => {
            new_config(path)?;
        }
        Commands::Test => {
            let config = load_config(None)?;
            init_rayon(&config);
            test(&config)?;
        }
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
