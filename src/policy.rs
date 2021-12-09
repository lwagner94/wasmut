use crate::error::Result;

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

pub struct MutationPolicyBuilder {
    allowlist: Vec<String>,
}

pub struct MutationPolicy {
    allowlist: Vec<Regex>,
    allow_all: bool,
}

impl MutationPolicyBuilder {
    pub fn new() -> Self {
        Self {
            allowlist: Vec::new(),
        }
    }

    pub fn allow_function<T: AsRef<str>>(mut self, func_name: T) -> Self {
        self.allowlist.push(String::from(func_name.as_ref()));
        self
    }

    pub fn build(self) -> Result<MutationPolicy> {
        let mut allowlist = Vec::new();

        for allowed in self.allowlist {
            let regex = Regex::new(&allowed)?;
            allowlist.push(regex);
        }

        Ok(MutationPolicy {
            allowlist,
            allow_all: false,
        })
    }
}

impl Default for MutationPolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MutationPolicy {
    pub fn check_function<T: AsRef<str>>(&self, func_name: T) -> bool {
        self.allow_all
            || self
                .allowlist
                .iter()
                .any(|regex| regex.is_match(func_name.as_ref()))
    }
}

impl Default for MutationPolicy {
    fn default() -> Self {
        Self {
            allowlist: Default::default(),
            allow_all: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_allowlist_trivial() -> Result<()> {
        let policy = MutationPolicyBuilder::new()
            .allow_function("test")
            .build()
            .unwrap();

        assert!(policy.check_function("test"));
        assert!(policy.check_function("invatestlid"));
        assert!(!policy.check_function("invalid"));

        Ok(())
    }
    #[test]
    fn build_allowlist_multiple() -> Result<()> {
        let policy = MutationPolicyBuilder::new()
            .allow_function("test")
            .allow_function("another")
            .build()
            .unwrap();

        assert!(policy.check_function("test"));
        assert!(policy.check_function("another"));

        Ok(())
    }

    #[test]
    fn build_allowlist_regex() -> Result<()> {
        let policy = MutationPolicyBuilder::new()
            .allow_function("^test_")
            .build()
            .unwrap();

        assert!(policy.check_function("test_func1"));
        assert!(policy.check_function("test_func2"));

        Ok(())
    }

    #[test]
    fn build_allowlist_empty() -> Result<()> {
        let policy = MutationPolicyBuilder::new().build().unwrap();

        assert!(!policy.check_function("test_func1"));
        assert!(!policy.check_function("test_func2"));

        Ok(())
    }
}
