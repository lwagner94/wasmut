use std::ffi::OsString;

use clap::{AppSettings, ArgEnum, Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
pub struct CLIArguments {
    #[clap(subcommand)]
    pub command: CLICommand,
}

#[derive(Subcommand)]
pub enum CLICommand {
    /// List all functions of the binary
    ListFunctions {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: String,
    },
    /// List all files
    ListFiles {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: String,
    },
    /// Run mutants
    Mutate {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Number of threads
        #[clap(short, long)]
        threads: Option<usize>,

        #[clap(short, long, arg_enum, default_value_t=Output::Console)]
        report: Output,

        #[clap(short, long, default_value = "wasmut-report")]
        output: String,
        /// Path to the wasm module
        wasmfile: String,
    },
    /// Create new configuration file
    NewConfig {
        /// Path to the new configuration file
        path: Option<String>,
    },

    /// Run module without any mutations
    Run {
        /// Path to wasmut.toml configuration
        #[clap(short, long)]
        config: Option<String>,

        /// Attempt to load wasmut.toml from the same directory as the wasm module
        #[clap(short = 'C', long)]
        config_samedir: bool,

        /// Path to the wasm module
        wasmfile: String,
    },

    /// List all available mutation operators
    ListOperators {
        /// Path to wasmut.toml configuration
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
    HTML,
}

impl CLIArguments {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn parse_args_from<I, T>(itr: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::parse_from(itr)
    }
}
