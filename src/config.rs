use anyhow::{Context, Result};
use std::path::Path;

use crate::templates;
use serde::Deserialize;

pub const TIMEOUT_MULTIPLIER: f64 = 2.0;

#[derive(Deserialize, Default)]
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

#[derive(Deserialize, Default)]
pub struct EngineConfig {
    timeout_multiplier: Option<f64>,
    map_dirs: Option<Vec<(String, String)>>,
}

impl EngineConfig {
    pub fn timeout_multiplier(&self) -> f64 {
        self.timeout_multiplier.unwrap_or(TIMEOUT_MULTIPLIER)
    }

    pub fn map_dirs(&self) -> &[(String, String)] {
        //

        if let Some(map_dirs) = self.map_dirs.as_ref() {
            map_dirs.as_slice()
        } else {
            &[]
        }
    }
}

#[derive(Deserialize, Default)]
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
#[derive(Deserialize, Default)]
pub struct OperatorConfig {
    enabled_operators: Option<Vec<String>>,
}

impl OperatorConfig {
    pub fn enabled_operators(&self) -> Vec<String> {
        self.enabled_operators
            .clone()
            .unwrap_or_else(|| vec![String::new()])
    }
}

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

        if config.report.is_none() {
            config.report = Some(Default::default());
        }

        if config.operators.is_none() {
            config.operators = Some(Default::default());
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
