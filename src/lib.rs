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

use anyhow::Result;

use crate::cliarguments::{CLIArguments, CLICommand};
use colored::*;
use log::*;
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

fn mutate(wasmfile: &str, config: &Config) -> Result<()> {
    let module = WasmModule::from_file(wasmfile)?;
    let mutator = MutationEngine::new(config)?;
    let mutations = mutator.discover_mutation_positions(&module)?;

    //dbg!(&mutations);

    let executor = Executor::new(config);
    let results = executor.execute(&module, &mutations)?;

    // dbg!(outcomes);

    let executed_mutants = reporter::prepare_results(&module, mutations, results);

    let cli_reporter = reporter::CLIReporter::new(config.report())?;

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
        let path_str = config_file.to_str().unwrap().into();

        let args = CLIArguments {
            command: CLICommand::NewConfig {
                path: Some(path_str),
            },
        };

        assert!(run_main(args).is_ok());
        assert!(config_file.exists());
    }

    fn mutate_and_check(testcase: &str) {
        let config_path = Path::new(&format!("testdata/{testcase}/wasmut.toml"))
            .canonicalize()
            .unwrap();
        let module_path = Path::new(&format!("testdata/{testcase}/test.wasm"))
            .canonicalize()
            .unwrap();

        let args = CLIArguments {
            command: CLICommand::Mutate {
                config: Some(config_path.to_str().unwrap().into()),
                wasmfile: module_path.to_str().unwrap().into(),
            },
        };

        assert!(run_main(args).is_ok());
        // TODO: Configure output directory.
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

        let args = CLIArguments {
            command: CLICommand::ListFunctions {
                config: Some(config_path.to_str().unwrap().into()),
                wasmfile: module_path.to_str().unwrap().into(),
            },
        };
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

        let args = CLIArguments {
            command: CLICommand::ListFiles {
                config: Some(config_path.to_str().unwrap().into()),
                wasmfile: module_path.to_str().unwrap().into(),
            },
        };
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
