use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_wasi::WasiState;

use crate::error::{Error, Result};
use crate::TestResult;
use crate::{runtime::Runtime, TestFunction};

use super::WasmModule;

pub struct WasmerRuntime {
    instance: wasmer::Instance,
}

impl Runtime for WasmerRuntime {
    fn new(module: WasmModule) -> Result<Self> {
        use wasmer::{Instance, Module, Store};

        let store = Store::new(&Universal::new(Cranelift::default()).engine());
        let bytecode: Vec<u8> = module.try_into()?;
        let module = Module::new(&store, &bytecode)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        let mut wasi_env = WasiState::new("command-name")
            .finalize()
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        let import_object = wasi_env
            .import_object(&module)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;
        let instance = Instance::new(&module, &import_object)
            .map_err(|e| Error::RuntimeCreation { source: e.into() })?;

        Ok(WasmerRuntime { instance })
    }

    fn call_returning_i32(&mut self, name: &str) -> Result<i32> {
        let func = self
            .instance
            .exports
            .get_function(name)
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        let native_func = func
            .native::<(), i32>()
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        native_func
            .call()
            .map_err(|e| Error::RuntimeCall { source: e.into() })
    }

    fn discover_test_functions(&mut self) -> Result<Vec<TestFunction>> {
        let mut test_functions = Vec::new();

        for (name, func) in self.instance.exports.iter() {
            if let wasmer::Extern::Function(f) = func {
                if f.native::<(), i32>().is_ok() {
                    test_functions.push(TestFunction {
                        name: name.clone(),
                        expected_result: true,
                    });
                }
            }
        }
        Ok(test_functions)
    }

    fn call_test_function(&mut self, test_function: &TestFunction) -> Result<TestResult> {
        let name = test_function.name.as_str();

        let func = self
            .instance
            .exports
            .get_function(name)
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        let native_func = func
            .native::<(), i32>()
            .map_err(|e| Error::RuntimeCall { source: e.into() })?;

        match native_func.call() {
            Ok(result) => {
                if (result != 0) == test_function.expected_result {
                    Ok(TestResult::Success)
                } else {
                    Ok(TestResult::Failure)
                }
            }
            Err(_) => {
                // TODO: Trap reason
                Ok(TestResult::Trapped)
            }
        }
    }
}
