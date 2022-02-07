use regex::Regex;

use anyhow::{Context, Result};
pub struct PathRewriter {
    regex: Regex,
    replacement: String,
}

impl PathRewriter {
    pub fn new<T: AsRef<str>>(regex: T, replacement: T) -> Result<Self> {
        let regex = regex.as_ref();

        Ok(Self {
            regex: Regex::new(regex)
                .with_context(|| format!("Failed to compile path replacement regex \"{regex}\""))?,
            replacement: replacement.as_ref().into(),
        })
    }

    pub fn rewrite<T: AsRef<str>>(&self, path: T) -> String {
        self.regex.replace(path.as_ref(), &self.replacement).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_rewriter_simple_replace() {
        let rewriter = PathRewriter::new("/home/lukas/", "").unwrap();

        assert_eq!(
            rewriter.rewrite("/home/lukas/wasmut/test.wasm"),
            "wasmut/test.wasm"
        );
    }
}
