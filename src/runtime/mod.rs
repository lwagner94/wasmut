pub mod wasmer;
pub mod wasmtime;

use anyhow::Result;

pub use crate::runtime::wasmer::WasmerRuntime;
pub use crate::runtime::wasmtime::WasmtimeRuntime;

pub trait Runtime {
    fn new(bytecode: &[u8]) -> Result<Self>
    where
        Self: Sized;
    fn call_returning_i32(&mut self, name: &str) -> Result<i32>;
}
