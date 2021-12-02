use anyhow::Result;

use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_wasi::WasiState;

use crate::runtime::Runtime;

pub struct WasmerRuntime {
    instance: wasmer::Instance,
}

impl Runtime for WasmerRuntime {
    fn new(bytecode: &[u8]) -> Result<Self> {
        use wasmer::{Instance, Module, Store};

        let store = Store::new(&Universal::new(Cranelift::default()).engine());
        let module = Module::new(&store, &bytecode)?;

        let mut wasi_env = WasiState::new("command-name").finalize()?;

        let import_object = wasi_env.import_object(&module)?;
        let instance = Instance::new(&module, &import_object)?;

        Ok(WasmerRuntime { instance })
    }

    fn call_returning_i32(&mut self, name: &str) -> Result<i32> {
        let func = self
            .instance
            .exports
            .get_function(name)?
            .native::<(), i32>()?;

        Ok(func.call()?)
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use std::fs::read;

    #[test]
    fn test_simple_add() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(&bytecode)?;
        let result = runtime.call_returning_i32("test_add_1")?;
        assert_eq!(result, 1);
        Ok(())
    }
}