use env_logger::Builder;
use log::{error, LevelFilter};
use wasmut::{cliarguments::CLIArguments, run_main};

fn main() {
    let cli = CLIArguments::parse_args();

    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .format_target(false)
        .filter_module("wasmer_wasi", LevelFilter::Warn)
        .init();

    match run_main(cli) {
        Ok(_) => {}
        Err(e) => {
            error!("{e:?}");
            std::process::exit(1);
        }
    }
}
