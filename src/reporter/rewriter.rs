use regex::Regex;

use crate::error::Result;

pub struct PathRewriter {
    regex: Regex,
    replacement: String,
}

impl PathRewriter {
    pub fn new<T: AsRef<str>>(regex: T, replacement: T) -> Result<Self> {
        Ok(Self {
            regex: Regex::new(regex.as_ref())?,
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
