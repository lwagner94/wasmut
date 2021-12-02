use anyhow::Result;

use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_wasi::WasiState;

use crate::{runtime::Runtime, TestFunction};

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

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>> {
        let mut test_functions = Vec::new();

        for (name, func) in self.instance.exports.iter() {
            if let wasmer::Extern::Function(f) = func {
                if f.native::<(), i32>().is_ok() {
                    test_functions.push(TestFunction {
                        name: name.clone(),
                    });
                }
            }
        }
        Ok(test_functions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::read;

    // TODO: See if it makes sense to generalize tests for both runtimes?

    #[test]
    fn test_simple_add() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(&bytecode)?;
        let result = runtime.call_returning_i32("test_add_1")?;
        assert_eq!(result, 1);
        Ok(())
    }

    #[test]
    fn test_discover_test_functions() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        let mut runtime = WasmerRuntime::new(&bytecode)?;
        let test_functions = runtime.discover_test_functions()?;
        assert_eq!(test_functions.len(), 2);
        Ok(())
    }
}
