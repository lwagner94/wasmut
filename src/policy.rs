use crate::{config::Config, error::Result};

use regex::Regex;

pub enum ExecutionPolicy {
    // Run the function until the execution limit is reached
    RunUntilLimit {
        // The maximum number of instructions to execute
        limit: u64,
    },
    // Run the function until the function returns
    RunUntilReturn,
}

#[derive(Debug, Clone)]
pub struct MutationPolicyBuilder {
    allowed_functions: RegexListBuilder,
    allowed_files: RegexListBuilder,
}

#[derive(Debug)]
pub struct MutationPolicy {
    allowed_functions: RegexList,
    allowed_files: RegexList,
}

impl MutationPolicyBuilder {
    pub fn new() -> Self {
        Self {
            allowed_functions: RegexListBuilder::new(),
            allowed_files: RegexListBuilder::new(),
        }
    }

    pub fn allow_function<T: AsRef<str>>(self, name: T) -> Self {
        Self {
            allowed_functions: self.allowed_functions.push(name),
            ..self
        }
    }

    pub fn allow_file<T: AsRef<str>>(self, name: T) -> Self {
        Self {
            allowed_files: self.allowed_files.push(name),
            ..self
        }
    }

    pub fn build(self) -> Result<MutationPolicy> {
        Ok(MutationPolicy {
            allowed_functions: self.allowed_functions.build()?,
            allowed_files: self.allowed_files.build()?,
        })
    }
}

impl Default for MutationPolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MutationPolicy {
    pub fn allow_all() -> Self {
        let mut builder = MutationPolicyBuilder::new();
        builder = builder.allow_function("");
        builder = builder.allow_file("");
        builder.build().unwrap()
    }

    pub fn from_config(config: &Config) -> Result<Self> {
        let mut builder = MutationPolicyBuilder::new();

        if let Some(allowed_files) = config
            .filter
            .as_ref()
            .and_then(|filter| filter.allowed_files.as_ref())
        {
            for file in allowed_files {
                builder = builder.allow_file(file);
            }
        }

        if let Some(allowed_functions) = config
            .filter
            .as_ref()
            .and_then(|filter| filter.allowed_functions.as_ref())
        {
            for function in allowed_functions {
                builder = builder.allow_function(function);
            }
        }

        builder.build()
    }

    pub fn check_function<T: AsRef<str>>(&self, name: T) -> bool {
        self.allowed_functions.any(name)
    }

    pub fn check_file<T: AsRef<str>>(&self, name: T) -> bool {
        self.allowed_files.any(name)
    }
}

#[derive(Debug, Clone)]
struct RegexListBuilder {
    regexes: Vec<String>,
}

#[derive(Debug, Clone)]
struct RegexList {
    regexes: Vec<Regex>,
}

impl RegexListBuilder {
    fn new() -> Self {
        Self {
            regexes: Vec::new(),
        }
    }

    fn push<T: AsRef<str>>(mut self, func_name: T) -> Self {
        self.regexes.push(String::from(func_name.as_ref()));
        self
    }

    fn build(self) -> Result<RegexList> {
        let mut allowlist = Vec::new();

        for allowed in self.regexes {
            let regex = Regex::new(&allowed)?;
            allowlist.push(regex);
        }

        Ok(RegexList { regexes: allowlist })
    }
}

impl RegexList {
    fn any<T: AsRef<str>>(&self, name: T) -> bool {
        self.regexes
            .iter()
            .any(|regex| regex.is_match(name.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::MutationFilter;

    use super::*;

    #[test]
    fn build_regexlist_trivial() -> Result<()> {
        let regex_list = RegexListBuilder::new().push("test").build().unwrap();

        assert!(regex_list.any("test"));
        assert!(regex_list.any("invatestlid"));
        assert!(!regex_list.any("invalid"));

        Ok(())
    }
    #[test]
    fn build_regexlist_multiple_regex() -> Result<()> {
        let policy = RegexListBuilder::new()
            .push("^test_")
            .push("another")
            .build()
            .unwrap();

        assert!(policy.any("test_func1"));
        assert!(policy.any("test_func2"));
        assert!(policy.any("another"));

        Ok(())
    }

    #[test]
    fn build_mutation_policy() -> Result<()> {
        let policy = MutationPolicyBuilder::new()
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
        let config = {
            let mut config: Config = Default::default();
            config.filter = Some(MutationFilter::default());
            let f = config.filter.as_mut().unwrap();
            f.allowed_functions = Some(vec!["^test_".into()]);
            f.allowed_files = Some(vec!["^src/".into()]);
            config
        };

        let policy = MutationPolicy::from_config(&config)?;

        assert!(policy.check_function("test_func1"));
        assert!(policy.check_function("test_func2"));
        assert!(policy.check_file("src/foo.rs"));
        assert!(!policy.check_file("test/foo.rs"));

        Ok(())
    }
}
