pub mod addressresolver;
pub mod cliarguments;
pub mod config;
pub mod defaults;
pub mod executor;
pub mod mutation;
pub mod operator;
pub mod output;
pub mod policy;
pub mod reporter;
pub mod runtime;
pub mod templates;
pub mod wasmmodule;

use anyhow::{bail, Context, Result};
use cliarguments::Output;

use crate::cliarguments::{CLIArguments, CLICommand};
use colored::*;
use log::*;
use reporter::Reporter;
use std::path::Path;

use crate::{
    config::Config, executor::Executor, mutation::MutationEngine, policy::MutationPolicy,
    wasmmodule::WasmModule,
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
        output::output_string(format!("{check_result_str}{function}\n"));
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
        output::output_string(format!("{check_result_str}{file}\n"));
    }

    Ok(())
}

fn mutate(
    wasmfile: &str,
    config: &Config,
    report_type: &Output,
    output_directory: &str,
) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    //dbg!(&mutations);

    let executor = Executor::new(config);
    let results = executor.execute(&module, &mutations)?;

    // dbg!(outcomes);

    let executed_mutants = reporter::prepare_results(&module, mutations, results);

    match report_type {
        Output::Console => {
            let cli_reporter = reporter::CLIReporter::new(config.report())?;
            cli_reporter.report(&executed_mutants)?;
        }
        Output::HTML => {
            reporter::generate_html(config, &executed_mutants, output_directory)?;
        }
    }

    Ok(())
}

fn new_config(path: Option<String>) -> Result<()> {
    let path = path.unwrap_or_else(|| "wasmut.toml".into());
    Config::save_default_config(&path)?;
    info!("Created new configuration file {path}");
    Ok(())
}

fn load_config(config_path: Option<String>, module: &str, config_samedir: bool) -> Result<Config> {
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
        let module_directory = Path::new(module)
            .parent()
            .context("wasmmodule has no parent path")?;
        let config_path = module_directory.join("wasmut.toml");
        info!("Loading configuration file from module directory: {config_path:?}");
        Ok(Config::parse_file(config_path)?)
    } else {
        let default_path = Path::new("wasmut.toml");

        if default_path.exists() {
            // wasmut.toml exists in current directory
            info!("Loading default configuration file {config_path:?}");
            Ok(Config::parse_file(default_path)?)
        } else {
            // No config found, using defaults
            info!("No configuration file found or specified, using default config");
            Ok(Config::default())
        }
    }
}

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

pub fn run_main(cli: CLIArguments) -> Result<()> {
    match cli.command {
        CLICommand::ListFunctions {
            config,
            wasmfile,
            config_samedir,
        } => {
            let config = load_config(config, &wasmfile, config_samedir)?;
            list_functions(&wasmfile, &config)?;
        }
        CLICommand::ListFiles {
            config,
            wasmfile,
            config_samedir,
        } => {
            let config = load_config(config, &wasmfile, config_samedir)?;
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
            dbg!(&report);
            dbg!(&output);
            let config = load_config(config, &wasmfile, config_samedir)?;
            init_rayon(threads);
            mutate(&wasmfile, &config, &report, &output)?;
        }
        CLICommand::NewConfig { path } => {
            new_config(path)?;
        }
    }

    Ok(())
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

        assert!(run_main(args).is_ok());
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
}
