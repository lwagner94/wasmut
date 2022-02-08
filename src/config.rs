use anyhow::{Context, Result};
use std::path::Path;

use crate::{defaults::TIMEOUT_MULTIPLIER, templates};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct FilterConfig {
    allowed_files: Option<Vec<String>>,
    allowed_functions: Option<Vec<String>>,
}

impl FilterConfig {
    pub fn allowed_files(&self) -> Option<&Vec<String>> {
        self.allowed_files.as_ref()
    }

    pub fn allowed_functions(&self) -> Option<&Vec<String>> {
        self.allowed_functions.as_ref()
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct EngineConfig {
    timeout_multiplier: Option<f64>,
}

impl EngineConfig {
    pub fn timeout_multiplier(&self) -> f64 {
        self.timeout_multiplier.unwrap_or(TIMEOUT_MULTIPLIER)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReportConfig {
    path_rewrite: Option<(String, String)>,
}

impl ReportConfig {
    pub fn path_rewrite(&self) -> Option<(&str, &str)> {
        self.path_rewrite
            .as_ref()
            .map(|(regex, replacement)| (regex.as_ref(), replacement.as_ref()))
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    engine: Option<EngineConfig>,
    filter: Option<FilterConfig>,
    report: Option<ReportConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            engine: Some(Default::default()),
            filter: Some(Default::default()),
            report: Some(Default::default()),
        }
    }
}

impl Config {
    pub fn parse_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }

    pub fn save_default_config<P: AsRef<Path>>(path: P) -> Result<()> {
        let p = path.as_ref();
        std::fs::write(p, templates::DEFAULT_CONFIG)
            .with_context(|| format!("Failed to write configuration file {p:?}"))?;
        Ok(())
    }

    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();

        let s = std::fs::read_to_string(p)
            .with_context(|| format!("Failed to read configuration file {p:?}"))?;

        Self::parse(&s)
    }

    fn parse(s: &str) -> Result<Self> {
        let mut config: Config = toml::from_str(s)?;

        if config.engine.is_none() {
            config.engine = Some(Default::default());
        }

        if config.filter.is_none() {
            config.filter = Some(Default::default());
        }
        Ok(config)
    }

    pub fn engine(&self) -> &EngineConfig {
        self.engine.as_ref().unwrap()
    }

    pub fn filter(&self) -> &FilterConfig {
        self.filter.as_ref().unwrap()
    }

    pub fn report(&self) -> &ReportConfig {
        self.report.as_ref().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn filters() -> Result<()> {
        let filter: FilterConfig = toml::from_str(
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
    fn engine_config() -> Result<()> {
        let engine: EngineConfig = toml::from_str(
            r#"
        timeout_multiplier = 2

    "#,
        )?;
        assert_eq!(engine.timeout_multiplier, Some(2.0));
        Ok(())
    }

    #[test]
    fn report_config() -> Result<()> {
        let module: ReportConfig = toml::from_str(
            r#"
        path_rewrite = ["foo", "bar"]
    "#,
        )?;
        assert_eq!(
            module.path_rewrite,
            Some((String::from("foo"), String::from("bar")))
        );
        Ok(())
    }

    #[test]
    fn save_default_config_is_created() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("wasmut.toml");
        Config::save_default_config(&file_path)?;
        assert!(file_path.exists());
        Ok(())
    }

    #[test]
    fn save_default_config_is_parsed_correctly() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("wasmut.toml");
        Config::save_default_config(&file_path)?;

        assert!(Config::parse_file(&file_path).is_ok());
        Ok(())
    }
}
