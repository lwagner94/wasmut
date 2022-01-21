use std::path::Path;

use crate::error::{Error, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct ModuleConfig {
    pub wasmfile: String,
}

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct MutationFilterConfig {
    pub allowed_files: Option<Vec<String>>,
    pub allowed_functions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct EngineConfig {
    pub threads: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct Config {
    pub module: ModuleConfig,
    pub engine: EngineConfig,
    pub filter: MutationFilterConfig,
}

impl Config {
    pub fn parse_str(s: &str) -> Result<Self> {
        Self::parse(s, Path::new("."))
    }

    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().is_file() || !path.as_ref().exists() {
            return Err(Error::FileNotFoundError(
                path.as_ref().to_str().expect("Invalid unicode").to_owned(),
            ));
        }

        let s = std::fs::read_to_string(&path).map_err(|e| Error::IOError { source: e })?;
        let parent = path.as_ref().parent().unwrap();

        Self::parse(&s, parent)
    }

    fn parse(s: &str, location: &Path) -> Result<Self> {
        let config = toml::from_str(s).map_err(|e| e.into());

        config.map(|mut c: Config| {
            // Fix path to wasmfile
            // If launch `wasmut` with the -c/--config flag, we can specify a
            // configuration file to be used.
            // In the config, the path in `wasmfile` is interpreted relative
            // to the location of the configuration file.
            // Thus we need to add the parent directory of the config
            // as a prefix.

            let wasmfile_path = Path::new(&c.module.wasmfile);
            if wasmfile_path.is_relative() {
                // We only need to add the prefix if the path is relative
                let prefix_path = location.join(wasmfile_path);
                c.module.wasmfile = prefix_path.to_str().expect("Invalid unicode").to_owned();
            }

            c
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn filters() -> Result<()> {
        let filter: MutationFilterConfig = toml::from_str(
            r#"

        allowed_files = ["src/", "test/"]
        allowed_functions = ["simple_rust", "test"]
    "#,
        )?;

        assert_eq!(
            filter.allowed_files,
            Some(vec![String::from("src/"), String::from("test/")])
        );
        assert_eq!(
            filter.allowed_functions,
            Some(vec![String::from("simple_rust"), String::from("test")])
        );
        Ok(())
    }

    #[test]
    fn empty_config() -> Result<()> {
        let config = Config::parse_str(
            r#"
    "#,
        );
        assert!(config.is_err());
        Ok(())
    }

    #[test]
    fn engine_config() -> Result<()> {
        let engine: EngineConfig = toml::from_str(
            r#"
        threads = 4
    "#,
        )?;
        assert_eq!(engine.threads, Some(4));
        Ok(())
    }

    #[test]
    fn module_config() -> Result<()> {
        let module: ModuleConfig = toml::from_str(
            r#"
        wasmfile = "test.wasm"
    "#,
        )?;
        assert_eq!(module.wasmfile, "test.wasm".to_owned());
        Ok(())
    }

    #[test]
    fn parse_file_wasmfile_path_fix() -> Result<()> {
        let config = Config::parse_file("testdata/simple_add/wasmut.toml")?;
        assert_eq!(
            config.module.wasmfile,
            "testdata/simple_add/test.wasm".to_owned()
        );
        Ok(())
    }

    #[test]
    fn parse_str_wasmfile_path_fix() -> Result<()> {
        let s = std::fs::read_to_string("testdata/simple_add/wasmut.toml")?;
        let config = Config::parse_str(&s)?;
        assert_eq!(config.module.wasmfile, "./test.wasm".to_owned());
        Ok(())
    }
}
