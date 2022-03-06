mod addressresolver;
mod cliarguments;
mod config;
mod executor;
mod mutation;
mod operator;
mod output;
mod policy;
mod reporter;
mod runtime;
mod templates;
mod wasmmodule;

use env_logger::Builder;
use log::{error, LevelFilter};

use anyhow::{bail, Context, Result};
use cliarguments::Output;
use operator::OperatorRegistry;

use crate::cliarguments::{CLIArguments, CLICommand};
use colored::*;
use log::*;
use reporter::{cli::CLIReporter, html::HTMLReporter, Reporter};
use std::path::Path;

use crate::{
    config::Config, executor::Executor, mutation::MutationEngine, policy::MutationPolicy,
    wasmmodule::WasmModule,
};

/// List all functions of a given WebAssembly module.
fn list_functions(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let policy = MutationPolicy::from_config(config)?;

    for function in module.functions() {
        let check_result_str = if policy.check_function(&function) {
            "allowed: ".green()
        } else {
            "denied:  ".red()
        };

        // Use our own output method so that we can capture it in unit tests
        output::output_string(format!("{check_result_str}{function}\n"));
    }

    Ok(())
}

/// List all source files that were used to build a given WebAssembly module.
fn list_files(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let policy = MutationPolicy::from_config(config)?;

    for file in module.source_files() {
        let check_result_str = if policy.check_file(&file) {
            "allowed: ".green()
        } else {
            "denied:  ".red()
        };

        // Use our own output method so that we can capture it in unit tests
        output::output_string(format!("{check_result_str}{file}\n"));
    }

    Ok(())
}

/// List all mutation operators.
fn list_operators(config: &Config) -> Result<()> {
    let enabled_ops = config.operators().enabled_operators();
    let ops = enabled_ops.iter().map(String::as_str).collect::<Vec<_>>();

    let registry = OperatorRegistry::new(&ops)?;

    for op_name in registry.enabled_operators() {
        let check_result_str = "enabled:  ".green();
        // Use our own output method so that we can capture it in unit tests
        output::output_string(format!("{check_result_str}{op_name}\n"));
    }

    for op_name in registry.disabled_operators() {
        let check_result_str = "disabled: ".red();
        // Use our own output method so that we can capture it in unit tests
        output::output_string(format!("{check_result_str}{op_name}\n"));
    }

    Ok(())
}

/// Find, apply and execute mutations.
fn mutate(
    wasmfile: &str,
    config: &Config,
    report_type: &Output,
    output_directory: &str,
) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    let executor = Executor::new(config);
    let results = executor.execute_mutants(&module, &mutations)?;

    let executed_mutants = reporter::prepare_results(&module, results)?;

    match report_type {
        Output::Console => {
            let reporter = CLIReporter::new(config.report())?;
            reporter.report(&executed_mutants)?;
        }
        Output::Html => {
            let reporter = HTMLReporter::new(config.report(), Path::new(output_directory))?;
            reporter.report(&executed_mutants)?;
        }
    }

    Ok(())
}

/// Find, apply and execute mutations.
fn mutate_meta(
    wasmfile: &str,
    config: &Config,
    report_type: &Output,
    output_directory: &str,
) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    let executor = Executor::new(config);
    let results = executor.execute_mutants_meta(&module, &mutations)?;

    let executed_mutants = reporter::prepare_results(&module, results)?;

    match report_type {
        Output::Console => {
            let reporter = CLIReporter::new(config.report())?;
            reporter.report(&executed_mutants)?;
        }
        Output::Html => {
            let reporter = HTMLReporter::new(config.report(), Path::new(output_directory))?;
            reporter.report(&executed_mutants)?;
        }
    }

    Ok(())
}

/// Create a new configuration file.
///
/// If `path` is `None`, a `wasmut.toml` file will be created in the current directory.
fn new_config(path: Option<String>) -> Result<()> {
    let path = path.unwrap_or_else(|| "wasmut.toml".into());
    Config::save_default_config(&path)?;
    info!("Created new configuration file {path}");
    Ok(())
}

/// Run a WebAssembly file without any mutations.
fn run(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let executor = Executor::new(config);
    executor.execute(&module)?;
    Ok(())
}

/// Load wasmut.toml configuration file.
fn load_config(
    config_path: Option<&str>,
    module: Option<&str>,
    config_samedir: bool,
) -> Result<Config> {
    if config_path.is_some() && config_samedir {
        bail!("Cannot use --config/-c and --config-same-dir/-C at the same time!");
    }

    if let Some(config_path) = config_path {
        // The user has supplied a configuration file
        info!("Loading user-specified configuration file {config_path:?}");
        Ok(Config::parse_file(config_path)?)
    } else if config_samedir {
        // The user has specified the -C option, indicating that wasmut should look for
        // a configuration file in the same directory as the module
        if module.is_none() {
            bail!("Cannot use --config-same-dir/-C without specifying a module!");
        }

        let module = module.unwrap();

        let module_directory = Path::new(&module)
            .parent()
            .context("wasmmodule has no parent path")?;
        let config_path = module_directory.join("wasmut.toml");
        info!("Loading configuration file from module directory: {config_path:?}");
        Ok(Config::parse_file(config_path)?)
    } else {
        let default_path = Path::new("wasmut.toml");

        if default_path.exists() {
            // wasmut.toml exists in current directory
            info!("Loading default configuration file {default_path:?}");
            Ok(Config::parse_file(default_path)?)
        } else {
            // No config found, using defaults
            info!("No configuration file found or specified, using default config");
            Ok(Config::default())
        }
    }
}

/// Initialize rayon thread pool
fn init_rayon(threads: Option<usize>) {
    let threads = threads.unwrap_or_else(num_cpus::get);

    info!("Using {threads} workers");

    // We ignore the error, because during
    // integration testing we might
    // call this functions twice in a process.
    // build_global only seems to return an error
    // if called twice, so this should be fine.
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads as usize)
        .build_global();
}

/// Implementation of main
fn run_main(cli: CLIArguments) -> Result<()> {
    match cli.command {
        CLICommand::ListFunctions {
            config,
            wasmfile,
            config_samedir,
        } => {
            let config = load_config(config.as_deref(), Some(&wasmfile), config_samedir)?;
            list_functions(&wasmfile, &config)?;
        }
        CLICommand::ListFiles {
            config,
            wasmfile,
            config_samedir,
        } => {
            let config = load_config(config.as_deref(), Some(&wasmfile), config_samedir)?;
            list_files(&wasmfile, &config)?;
        }
        CLICommand::Mutate {
            config,
            wasmfile,
            threads,
            config_samedir,
            report,
            output,
        } => {
            let config = load_config(config.as_deref(), Some(&wasmfile), config_samedir)?;
            init_rayon(threads);
            mutate(&wasmfile, &config, &report, &output)?;
        }
        CLICommand::MutateMeta {
            config,
            wasmfile,
            threads,
            config_samedir,
            report,
            output,
        } => {
            let config = load_config(config.as_deref(), Some(&wasmfile), config_samedir)?;
            init_rayon(threads);
            mutate_meta(&wasmfile, &config, &report, &output)?;
        }
        CLICommand::NewConfig { path } => {
            new_config(path)?;
        }
        CLICommand::Run {
            config,
            config_samedir,
            wasmfile,
        } => {
            let config = load_config(config.as_deref(), Some(&wasmfile), config_samedir)?;
            run(&wasmfile, &config)?;
        }
        CLICommand::ListOperators {
            config,
            config_samedir,
            wasmfile,
        } => {
            let config = load_config(config.as_deref(), wasmfile.as_deref(), config_samedir)?;
            list_operators(&config)?;
        }
    }

    Ok(())
}

/// Actual main function
fn main() {
    let cli = CLIArguments::parse_args();

    Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_timestamp(None)
        // .format_target(false)
        .filter_module("wasmer_wasi", LevelFilter::Warn)
        .filter_module("regalloc", LevelFilter::Warn)
        .filter_module("cranelift_codegen", LevelFilter::Warn)
        .filter_module("wasmer_compiler_cranelift", LevelFilter::Warn)
        .init();

    match run_main(cli) {
        Ok(_) => {}
        Err(e) => {
            error!("{e:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_config_is_created_standard_path() {
        let args = CLIArguments {
            command: CLICommand::NewConfig { path: None },
        };

        assert!(run_main(args).is_ok());
        let config_file = Path::new("wasmut.toml");
        assert!(config_file.exists());
        assert!(Config::parse_file(config_file).is_ok());

        std::fs::remove_file("wasmut.toml").unwrap();
    }

    #[test]
    fn new_config_is_created_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let config_file = dir.path().join("custom.toml");
        let path_str = config_file.to_str().unwrap();

        let args = CLIArguments::parse_args_from(vec!["wasmut", "new-config", path_str]);

        assert!(run_main(args).is_ok());
        assert!(config_file.exists());
    }

    fn mutate_and_check(testcase: &str) {
        let module_path = Path::new(&format!("testdata/{testcase}/test.wasm"))
            .canonicalize()
            .unwrap();

        let output_dir = tempfile::tempdir().unwrap();

        let output_dir_str = output_dir.path().to_str().unwrap();

        let args = CLIArguments::parse_args_from(vec![
            "wasmut",
            "mutate",
            "-C",
            "-r",
            "html",
            "-o",
            output_dir_str,
            module_path.to_str().unwrap(),
        ]);
        let result = run_main(args);
        // dbg!(&result);
        assert!(result.is_ok());
        assert!(output_dir.path().join("index.html").exists());
    }

    #[test]
    fn test_mutations() {
        mutate_and_check("simple_add");
        mutate_and_check("factorial");
    }

    #[test]
    fn test_list_functions() {
        let config_path = Path::new("testdata/simple_add/wasmut.toml");
        let module_path = Path::new("testdata/simple_add/test.wasm");

        let args = CLIArguments::parse_args_from(vec![
            "wasmut",
            "list-functions",
            "-c",
            config_path.to_str().unwrap(),
            module_path.to_str().unwrap(),
        ]);

        output::clear_output();
        assert!(run_main(args).is_ok());

        let command_output = output::get_output();
        let a = command_output.split('\n');

        for line in a {
            assert!(
                (line.contains(" add ") && line.contains("allowed")
                    || !(line.contains(" add ") && line.contains("denied")))
            )
        }
    }

    #[test]
    fn test_list_files() {
        let config_path = Path::new("testdata/simple_add/wasmut_files.toml");
        let module_path = Path::new("testdata/simple_add/test.wasm");

        let args = CLIArguments::parse_args_from(vec![
            "wasmut",
            "list-files",
            "-c",
            config_path.to_str().unwrap(),
            module_path.to_str().unwrap(),
        ]);
        output::clear_output();
        assert!(run_main(args).is_ok());

        let command_output = output::get_output();
        let a = command_output.split('\n');

        let mut hits = 0;

        for line in a {
            if line.contains("denied") && (line.contains("test.c"))
                || (line.contains("simple_add.c") && line.contains("allowed"))
            {
                hits += 1;
            };
        }

        assert_eq!(hits, 2);
    }

    fn run_module(testcase: &str) -> Result<()> {
        let path_string = format!("testdata/{testcase}/test.wasm");
        let module_path = Path::new(&path_string);

        let args = CLIArguments::parse_args_from(vec![
            "wasmut",
            "run",
            "-C",
            module_path.to_str().unwrap(),
        ]);

        run_main(args)
    }

    #[test]
    fn test_run_zero_exit() {
        assert!(run_module("simple_add").is_ok());
    }

    #[test]
    fn test_run_nonzero_exit() {
        assert!(run_module("nonzero_exit").is_err());
    }

    #[test]
    fn test_run_count_words() {
        // Test the map_dirs parameter
        assert!(run_module("count_words").is_ok());
    }

    #[test]
    fn test_list_operators() {
        let config_path = Path::new("testdata/count_words/wasmut_call.toml");

        let args = CLIArguments::parse_args_from(vec![
            "wasmut",
            "list-operators",
            "-c",
            config_path.to_str().unwrap(),
        ]);
        output::clear_output();
        assert!(run_main(args).is_ok());

        let command_output = output::get_output();
        let lines = command_output.split('\n');

        let mut counted_operators = 0;

        for line in lines {
            if line.contains("enabled") && (line.contains("call_"))
                || (line.contains("disabled")
                    && (line.contains("binop_")
                        || line.contains("unop_")
                        || line.contains("relop")
                        || line.contains("const_")))
            {
                counted_operators += 1;
            };
        }

        assert_eq!(counted_operators, 31);
    }
}
