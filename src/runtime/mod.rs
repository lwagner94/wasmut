pub mod wasmer;
pub mod wasmtime;
pub use crate::runtime::wasmer::WasmerRuntime;
pub use crate::runtime::wasmtime::WasmtimeRuntime;

use anyhow::Result;

use crate::TestFunction;

pub trait Runtime {
    fn new(bytecode: &[u8]) -> Result<Self>
    where
        Self: Sized;
    fn call_returning_i32(&mut self, name: &str) -> Result<i32>;

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>>;
}
