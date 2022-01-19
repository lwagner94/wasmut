use std::path::Path;

use crate::error::{Error, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct MutationFilter {
    pub allowed_files: Option<Vec<String>>,
    pub allowed_functions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct Config {
    pub filter: Option<MutationFilter>,
}

impl Config {
    pub fn parse_str(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| e.into())
    }

    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = std::fs::read_to_string(path).map_err(|e| Error::IOError { source: e })?;

        Self::parse_str(&config_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use anyhow::Result;

    #[test]
    fn filters() -> Result<()> {
        let config = Config::parse_str(
            r#"
        [filter]
        allowed_files = ["src/", "test/"]
        allowed_functions = ["simple_rust", "test"]
    "#,
        )?;
        let filter = config.filter.unwrap();
        assert_eq!(
            filter.allowed_files,
            Some(vec!["src/".into(), "test/".into()])
        );
        assert_eq!(
            filter.allowed_functions,
            Some(vec!["simple_rust".into(), "test".into()])
        );
        Ok(())
    }

    #[test]
    fn no_filters() -> Result<()> {
        let config = Config::parse_str(
            r#"
    "#,
        )?;
        let f = config.filter;
        assert!(f.is_none());
        Ok(())
    }
}
