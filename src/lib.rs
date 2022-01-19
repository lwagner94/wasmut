pub mod addressresolver;
pub mod config;
pub mod error;
pub mod operator;
pub mod policy;
pub mod runtime;
pub mod wasmmodule;

use config::Config;
use error::Result;

#[derive(Debug)]
pub struct TestFunction {
    pub name: String,
    pub expected_result: bool,
    pub function_type: TestFunctionType,
}

#[derive(Debug)]
pub enum TestFunctionType {
    StartEntryPoint,
    FuncReturningI32,
}

#[derive(Debug)]
pub enum ExecutionResult {
    // Normal termination
    FunctionReturn { return_value: i32 },
    ProcessExit { exit_code: u32 },
    // Execution limit exceeded
    LimitExceeded,

    // Other error
    Error,
}

// TODO: Move this somewhere else?
pub fn load_config(wasmfile: &str, config: Option<&str>) -> Result<Config> {
    use std::path::Path;

    //let in_dir_config = Path::new("./wasmut.toml");
    // TODO: Better error handling
    // TODO: Support ./wasmut.toml file in current directory
    let wasmfile_config = Path::new(wasmfile).parent().unwrap().join("wasmut.toml");

    if let Some(path) = config {
        Config::parse_file(path)
    } else {
        log::debug!("Trying configuration file {wasmfile_config:?}");
        Config::parse_file(wasmfile_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_load_config_config_alongside_wasm() -> Result<()> {
        let c = load_config("testdata/simple_add/test.wasm", None);
        assert!(c.is_ok());
        Ok(())
    }
}
