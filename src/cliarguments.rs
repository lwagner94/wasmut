use clap::{AppSettings, Parser, Subcommand};

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
    NewConfig {
        path: Option<String>,
    },
}

impl CLIArguments {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
