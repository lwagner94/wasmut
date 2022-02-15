use anyhow::{Context, Result};
use std::path::Path;

use crate::templates;
use serde::Deserialize;

/// Default value for the `timeout_multiplier` configuration key
pub const TIMEOUT_MULTIPLIER: f64 = 2.0;

/// Configuration for mutant filtering.
#[derive(Deserialize, Default)]
pub struct FilterConfig {
    /// Regex list of all files that should be mutated
    allowed_files: Option<Vec<String>>,

    /// Regex list of all functions that should be mutated
    allowed_functions: Option<Vec<String>>,
}

impl FilterConfig {
    /// Get list of regular expressions of all files that should be mutated
    pub fn allowed_files(&self) -> Option<&Vec<String>> {
        self.allowed_files.as_ref()
    }

    /// Get list of regular expressions of all functions that should be mutated
    pub fn allowed_functions(&self) -> Option<&Vec<String>> {
        self.allowed_functions.as_ref()
    }
}

/// Configuration for the execution engine
#[derive(Deserialize, Default)]
pub struct EngineConfig {
    /// Execution timeout multiplier. timeout will be
    /// set to cycles measured in baseline run multiplied by this factor
    timeout_multiplier: Option<f64>,

    /// A list of all directories that are to be mapped into the runtime
    map_dirs: Option<Vec<(String, String)>>,
}

impl EngineConfig {
    /// Execution timeout multiplier
    pub fn timeout_multiplier(&self) -> f64 {
        self.timeout_multiplier.unwrap_or(TIMEOUT_MULTIPLIER)
    }

    /// A list of all directories that are to be mapped into the runtime
    pub fn map_dirs(&self) -> &[(String, String)] {
        if let Some(map_dirs) = self.map_dirs.as_ref() {
            map_dirs.as_slice()
        } else {
            &[]
        }
    }
}

/// Configuration regarding report generation
#[derive(Deserialize, Default)]
pub struct ReportConfig {
    /// Rewrite paths using Regex::replace
    path_rewrite: Option<(String, String)>,
}

impl ReportConfig {
    /// Return path replacement configuration
    pub fn path_rewrite(&self) -> Option<(&str, &str)> {
        self.path_rewrite
            .as_ref()
            .map(|(regex, replacement)| (regex.as_ref(), replacement.as_ref()))
    }
}

/// Configuration for mutation operators
#[derive(Deserialize, Default)]
pub struct OperatorConfig {
    /// (Regex) list of all enabled mutation operators
    enabled_operators: Option<Vec<String>>,
}

impl OperatorConfig {
    /// Return a (regex) list of all enabled mutation operators
    pub fn enabled_operators(&self) -> Vec<String> {
        self.enabled_operators
            .clone()
            .unwrap_or_else(|| vec![String::new()])
    }
}

/// Main toml configuration
#[derive(Deserialize)]
pub struct Config {
    engine: Option<EngineConfig>,
    filter: Option<FilterConfig>,
    report: Option<ReportConfig>,
    operators: Option<OperatorConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            engine: Some(Default::default()),
            filter: Some(Default::default()),
            report: Some(Default::default()),
            operators: Some(Default::default()),
        }
    }
}

impl Config {
    /// Save default configuration to given path
    pub fn save_default_config<P: AsRef<Path>>(path: P) -> Result<()> {
        let p = path.as_ref();
        std::fs::write(p, templates::DEFAULT_CONFIG)
            .with_context(|| format!("Failed to write configuration file {p:?}"))?;
        Ok(())
    }

    /// Parse configuration at a given path
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();

        let s = std::fs::read_to_string(p)
            .with_context(|| format!("Failed to read configuration file {p:?}"))?;

        Self::parse(&s)
    }

    /// Parse configuration from string
    pub fn parse(s: &str) -> Result<Self> {
        let mut config: Config = toml::from_str(s)?;

        if config.engine.is_none() {
            config.engine = Some(Default::default());
        }

        if config.filter.is_none() {
            config.filter = Some(Default::default());
        }

        if config.report.is_none() {
            config.report = Some(Default::default());
        }

        if config.operators.is_none() {
            config.operators = Some(Default::default());
        }
        Ok(config)
    }

    /// Return engine subsection
    pub fn engine(&self) -> &EngineConfig {
        self.engine.as_ref().unwrap()
    }

    /// Return filter subsection
    pub fn filter(&self) -> &FilterConfig {
        self.filter.as_ref().unwrap()
    }

    /// Return report subsection
    pub fn report(&self) -> &ReportConfig {
        self.report.as_ref().unwrap()
    }

    /// Return operators subsection
    pub fn operators(&self) -> &OperatorConfig {
        self.operators.as_ref().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn filters() -> Result<()> {
        let config = Config::parse(
            r#"
            [filter]
            allowed_files = ["src/", "test/"]
            allowed_functions = ["simple_rust", "test"]
            "#,
        )?;

        assert_eq!(
            config.filter().allowed_files(),
            Some(&vec![String::from("src/"), String::from("test/")])
        );
        assert_eq!(
            config.filter().allowed_functions(),
            Some(&vec![String::from("simple_rust"), String::from("test")])
        );
        Ok(())
    }

    #[test]
    fn engine_config() -> Result<()> {
        let config = Config::parse(
            r#"
            [engine]
            timeout_multiplier = 10
            map_dirs = [["a/foo", "b/bar"], ["abcd", "abcd"]]
            "#,
        )?;
        assert_eq!(config.engine().timeout_multiplier(), 10.0);
        assert_eq!(
            config.engine().map_dirs(),
            [
                ("a/foo".into(), "b/bar".into()),
                ("abcd".into(), "abcd".into())
            ]
        );
        Ok(())
    }

    #[test]
    fn operator_config() -> Result<()> {
        let config = Config::parse(
            r#"
            [operators]
            enabled_operators = ["relop", "unop"]

            "#,
        )?;
        let expected: Vec<String> = vec!["relop".into(), "unop".into()];
        assert_eq!(config.operators().enabled_operators(), expected);
        Ok(())
    }

    #[test]
    fn report_config() -> Result<()> {
        let config = Config::parse(
            r#"
            [report]
            path_rewrite = ["foo", "bar"]
            "#,
        )?;
        assert_eq!(config.report().path_rewrite(), Some(("foo", "bar")));
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

    #[test]
    fn default_config() -> Result<()> {
        let config = Config::parse(
            r#"
            "#,
        )?;
        assert_eq!(config.engine().timeout_multiplier(), 2.0);
        assert_eq!(config.engine().map_dirs(), []);
        assert_eq!(config.filter().allowed_files(), None);
        assert_eq!(config.filter().allowed_functions(), None);
        assert_eq!(config.report().path_rewrite(), None);
        assert_eq!(
            config.operators().enabled_operators(),
            vec![String::from("")]
        );
        Ok(())
    }
}
