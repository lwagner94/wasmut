use crate::config::Config;

use anyhow::{Context, Result};

use regex::RegexSet;

pub enum ExecutionPolicy {
    // Run the function until the execution limit is reached
    RunUntilLimit {
        // The maximum number of instructions to execute
        limit: u64,
    },
    // Run the function until the function returns
    RunUntilReturn,
}

pub struct MutationPolicyBuilder {
    allowed_functions: Vec<String>,
    allowed_files: Vec<String>,
    anything_allowed: bool,
}

pub struct MutationPolicy {
    allowed_functions: RegexSet,
    allowed_files: RegexSet,
    anything_allowed: bool,
}

impl MutationPolicyBuilder {
    pub fn allow_function<T: AsRef<str>>(mut self, name: T) -> Self {
        self.allowed_functions.push(String::from(name.as_ref()));
        Self {
            anything_allowed: false,
            ..self
        }
    }

    pub fn allow_file<T: AsRef<str>>(mut self, name: T) -> Self {
        self.allowed_files.push(String::from(name.as_ref()));
        Self {
            anything_allowed: false,
            ..self
        }
    }

    pub fn build(self) -> Result<MutationPolicy> {
        let allowed_functions = RegexSet::new(&self.allowed_functions)
            .context("Could not build allowed_functions regex set")?;
        let allowed_files = RegexSet::new(&self.allowed_files)
            .context("Could not build allowed_files regex set")?;

        Ok(MutationPolicy {
            allowed_functions,
            allowed_files,
            anything_allowed: self.anything_allowed,
        })
    }
}

impl Default for MutationPolicyBuilder {
    fn default() -> Self {
        Self {
            allowed_functions: Default::default(),
            allowed_files: Default::default(),
            anything_allowed: true,
        }
    }
}

impl MutationPolicy {
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut builder = MutationPolicyBuilder::default();

        if let Some(files) = config.filter().allowed_files() {
            for file in files {
                builder = builder.allow_file(file);
            }
        }

        if let Some(functions) = config.filter().allowed_functions() {
            for function in functions {
                builder = builder.allow_function(function);
            }
        }

        builder.build()
    }

    pub fn check_function<T: AsRef<str>>(&self, name: T) -> bool {
        self.anything_allowed || self.allowed_functions.is_match(name.as_ref())
    }

    pub fn check_file<T: AsRef<str>>(&self, name: T) -> bool {
        self.anything_allowed || self.allowed_files.is_match(name.as_ref())
    }

    pub fn check<T: AsRef<str>>(&self, file: Option<T>, func: Option<T>) -> bool {
        let file_allowed = file.map_or(false, |file| self.check_file(file));
        let func_allowed = func.map_or(false, |func| self.check_function(func));

        file_allowed || func_allowed
    }
}

impl Default for MutationPolicy {
    fn default() -> Self {
        Self {
            allowed_functions: RegexSet::new(&[] as &[&str]).unwrap(),
            allowed_files: RegexSet::new(&[] as &[&str]).unwrap(),
            anything_allowed: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn build_mutation_policy() -> Result<()> {
        let policy = MutationPolicyBuilder::default()
            .allow_function("^test_")
            .allow_file("^src/")
            .build()
            .unwrap();

        assert!(policy.check_function("test_func1"));
        assert!(policy.check_function("test_func2"));
        assert!(policy.check_file("src/foo.rs"));
        assert!(!policy.check_file("test/foo.rs"));

        Ok(())
    }

    #[test]
    fn policy_from_config() -> Result<()> {
        let config = Config::parse(
            r#"
        [filter]
        allowed_functions = ["^test"]
        allowed_files = ["^src/"] "#,
        )?;

        let policy = MutationPolicy::from_config(&config)?;

        assert!(policy.check_function("test_func1"));
        assert!(policy.check_function("test_func2"));
        assert!(policy.check_file("src/foo.rs"));
        assert!(!policy.check_file("test/foo.rs"));

        Ok(())
    }

    #[test]
    fn empty_policy_allows_all() -> Result<()> {
        let policy = MutationPolicy::default();

        assert!(policy.check_function("test_func1"));
        assert!(policy.check_function("test_func2"));
        assert!(policy.check_file("src/foo.rs"));
        assert!(policy.check_file("test/foo.rs"));

        Ok(())
    }
}
