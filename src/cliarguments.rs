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

impl CLIArguments {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
