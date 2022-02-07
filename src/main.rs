use log::error;
use wasmut::{cliarguments::CLIArguments, run_main};

fn main() {
    let cli = CLIArguments::parse_args();

    match run_main(cli) {
        Ok(_) => {}
        Err(e) => {
            error!("{e:?}");
            std::process::exit(1);
        }
    }
}
