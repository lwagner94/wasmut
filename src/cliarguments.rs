use std::ffi::OsString;

use clap::{ArgEnum, Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct CLIArguments {
    #[clap(subcommand)]
    pub command: CLICommand,
}

#[derive(Subcommand)]
pub enum CLICommand {
    /// List all functions of the binary.
    ///
    /// If a config is provided, this command will also
    /// show whether the function is allowed to be mutated.
    /// By default, wasmut will try to load a wasmut.toml file from the current directory
    ListFunctions {
        /// Load wasmut.toml configuration file from the provided path
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: String,
    },
    /// List all files of the binary.
    ///
    /// If a config is provided, this command will also
    /// show whether the file is allowed to be mutated.
    /// By default, wasmut will try to load a wasmut.toml file from the current directory
    ListFiles {
        /// Load wasmut.toml configuration file from the provided path
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: String,
    },
    /// Generate and run mutants.
    ///
    /// Given a (possibly default) configuration, wasmut will attempt to discover
    /// mutants and subsequently execute them. After that, a report will be generated
    Mutate {
        /// Load wasmut.toml configuration file from the provided path
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Number of threads to use when executing mutants
        #[clap(short, long)]
        threads: Option<usize>,

        /// Report output format
        #[clap(short, long, arg_enum, default_value_t=Output::Console)]
        report: Output,

        /// Output directory for reports
        #[clap(short, long, default_value = "wasmut-report")]
        output: String,

        /// Path to the wasm module
        wasmfile: String,
    },
    /// Create new configuration file.
    NewConfig {
        /// Path to the new configuration file
        path: Option<String>,
    },

    /// Run module without any mutations.
    Run {
        /// Load wasmut.toml configuration file from the provided path
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: String,
    },

    /// List all available mutation operators.
    ///
    /// If a config is provided, this command will also
    /// show whether the operator is enabled or not.
    /// By default, wasmut will try to load a wasmut.toml file from the current directory
    ListOperators {
        /// Load wasmut.toml configuration file from the provided path
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: Option<String>,
    },
}

#[derive(ArgEnum, Clone, Debug)]
pub enum Output {
    Console,
    Html,
}

impl CLIArguments {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    #[allow(dead_code)]
    pub fn parse_args_from<I, T>(itr: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::parse_from(itr)
    }
}
